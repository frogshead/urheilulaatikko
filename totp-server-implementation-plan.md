# TOTP Server — toteutussuunnitelma

Tämä dokumentti on tarkoitettu syötettäväksi Claude Codelle projektin
scaffoldausta ja toteutusta varten. Se kuvaa arkkitehtuurin, tiedostorakenteen,
riippuvuudet, API-kontraktin, Docker-testiympäristön ja etenemisjärjestyksen
askel askeleelta niin, että jokainen vaihe on itsenäisesti testattavissa.

## 1. Konteksti ja rajaus

TOTP server on osa suurempaa järjestelmää, jossa käyttäjä autentikoituu
erilliseen Auth serveriin (Keycloak) ja saa JWT-access tokenin. Käyttäjä esittää
tämän tokenin TOTP serverille pyytäessään kertakäyttöisen PIN-koodin, jonka hän
syöttää käsin offline-lukkoon. TOTP server ei koskaan kommunikoi suoraan lukon
kanssa — se on täysin erillinen, verkoton laite.

TOTP server:

1. Ottaa vastaan pyynnön `Authorization: Bearer <JWT>` -headerilla.
2. Varmentaa tokenin Keycloakilta **RFC 7662 -introspektiolla** (ei paikallista
   JWT-signeerauksen tarkistusta — revocation-tuki on tässä tärkeämpi kuin
   nopeus, ks. aiempi keskustelu).
3. Hakee tokenin `sub`-claimia vastaavan käyttäjän TOTP-salaisuuden
   tietokannasta (salattuna levossa).
4. Laskee senhetkisen TOTP-koodin (RFC 6238) ja palauttaa sen.
5. Rajoittaa pyyntötiheyttä käyttäjäkohtaisesti (rate limiting).

Ei tämän suunnitelman piirissä: itse Keycloak-realmin tuotantokonfigurointi,
lukon firmware, käyttäjän provisiointi-UI. Näistä provisiointi-endpoint sisältyy
minimimuodossa, jotta testiympäristö on käytettävissä päästä päähän.

## 2. Teknologiavalinnat

| Tarve                        | Valinta                                              | Perustelu                                                                                             |
| ---------------------------- | ---------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| Web-framework                | `axum`                                               | Vaatimus                                                                                              |
| Async runtime                | `tokio`                                              | Axumin riippuvuus                                                                                     |
| HTTP-klientti introspektioon | `reqwest` (rustls-tls)                               | Vakiovalinta, tukee Basic auth                                                                        |
| TOTP-generointi              | `totp-rs`                                            | RFC 6238 -yhteensopiva, aktiivisesti ylläpidetty                                                      |
| Tietokanta                   | `sqlx` + PostgreSQL                                  | Sama Postgres-instanssi voi palvella myös Keycloakia dev-ympäristössä                                 |
| Salaus levossa               | `aes-gcm` + `rand`                                   | AEAD, yksinkertainen avaintenhallinta env-muuttujasta lähtien                                         |
| Konfiguraatio                | `config` + `serde`                                   | Ympäristömuuttujat + `.env` kehityksessä                                                              |
| Virheenkäsittely             | `thiserror` + `anyhow`                               | Selkeä virhetyyppien erottelu API-kerroksessa                                                         |
| Lokitus                      | `tracing` + `tracing-subscriber`                     | Rakenteellinen lokitus, JSON tuotannossa                                                              |
| Rate limiting                | `tower_governor` tai oma middleware Redis-laskurilla | Aloita `tower_governor`:lla per-IP/per-sub, Redis jos tarvitaan hajautettu laskenta                   |
| Testaus                      | `testcontainers` (Rust) + `wiremock`                 | Integraatiotestit oikeaa Keycloak-konttia vasten, Wiremock introspektion mockaukseen yksikkötesteissä |

## 3. Projektirakenne

```
totp-server/
├── Cargo.toml
├── .env.example
├── Dockerfile
├── docker-compose.yml
├── docker/
│   └── keycloak/
│       └── realm-export.json
├── migrations/
│   └── 0001_init.sql
├── src/
│   ├── main.rs
│   ├── config.rs
│   ├── error.rs
│   ├── state.rs
│   ├── routes/
│   │   ├── mod.rs
│   │   ├── health.rs
│   │   ├── totp.rs
│   │   └── provision.rs
│   ├── auth/
│   │   ├── mod.rs
│   │   ├── introspection.rs
│   │   └── middleware.rs
│   ├── totp/
│   │   ├── mod.rs
│   │   └── secret_store.rs
│   └── crypto.rs
└── tests/
    ├── totp_generation.rs
    └── introspection_integration.rs
```

## 4. Tietomalli

Taulu `totp_secrets`:

| Sarake             | Tyyppi             | Kuvaus                                |
| ------------------ | ------------------ | ------------------------------------- |
| `subject`          | `text primary key` | Keycloakin `sub`-claim (käyttäjän ID) |
| `encrypted_secret` | `bytea`            | AES-GCM-salattu TOTP-secret           |
| `nonce`            | `bytea`            | AEAD-nonce                            |
| `created_at`       | `timestamptz`      |                                       |
| `last_used_at`     | `timestamptz null` | Audit-lokitusta varten                |

Taulu `request_log` (valinnainen, mutta suositeltu audit-jälkeä varten):

| Sarake         | Tyyppi        | Kuvaus                                              |
| -------------- | ------------- | --------------------------------------------------- |
| `id`           | `uuid`        |                                                     |
| `subject`      | `text`        |                                                     |
| `requested_at` | `timestamptz` |                                                     |
| `result`       | `text`        | `issued` / `denied_inactive_token` / `rate_limited` |

## 5. API-kontrakti

### `POST /v1/totp/request`

- Header: `Authorization: Bearer <jwt>`
- 200 OK:
  ```json
  { "pin": "482913", "valid_for_seconds": 21 }
  ```
- 401 Unauthorized — token puuttuu, on vanhentunut tai introspektio palautti
  `active: false`
- 403 Forbidden — token on voimassa mutta `aud`-claim ei sisällä `totp-server`
- 404 Not Found — käyttäjälle ei ole rekisteröityä TOTP-secretiä
- 429 Too Many Requests — rate limit ylittyi

### `POST /v1/totp/provision` (admin-suojattu, vain testiympäristöön/bootstrapiin)

- Header: `Authorization: Bearer <admin-jwt>` (erillinen scope/rooli, esim.
  `totp-admin`)
- Body: `{ "subject": "keycloak-user-id" }`
- 200 OK: `{ "secret_base32": "...", "otpauth_url": "otpauth://totp/..." }`
  (palautetaan **vain kertaalleen** provisioinnin yhteydessä, ei koskaan
  myöhemmin)

### `GET /healthz`

- 200 OK, ei autentikointia — käytetään Docker healthcheckissä

## 6. RFC 7662 -introspektioklientti

`src/auth/introspection.rs` vastuulla:

1. Rakenna `POST {KEYCLOAK_ISSUER}/protocol/openid-connect/token/introspect`
2. Basic auth: `client_id`/`client_secret` ympäristömuuttujista
   (`TOTP_SERVER_KEYCLOAK_CLIENT_ID`, `TOTP_SERVER_KEYCLOAK_CLIENT_SECRET`)
3. Body: `token=<jwt>` (`application/x-www-form-urlencoded`)
4. Käsittele vastaus:
   - `active: false` tai HTTP-virhe → 401
   - `active: true` → tarkista `aud` sisältää odotetun arvon, tarkista `exp` ei
     ole mennyt (introspektio yleensä hoitaa tämän jo, tarkista silti
     puolustavasti)
   - Poimi `sub` jatkokäsittelyyn
5. **Ei pitkäaikaista cachea** vastaukselle (revocation-vaatimus), mutta lisää
   lyhyt in-flight-deduplikaatio (esim. 1-2s) samalle tokenille, jos
   samanaikaisia pyyntöjä tulee — estää turhaa kuormaa Keycloakille ilman että
   revocation-hyöty katoaa.
6. Verkkovirheet: palauta 503, älä koskaan fail-open (jos introspektiota ei
   saada, pyyntö hylätään — ei koskaan oleteta tokenin olevan voimassa).

Toteuta tämä Axum-middlewarena (`src/auth/middleware.rs`), joka asettaa
varmennetun `sub`:n `Extension`-arvoksi seuraaville handlereille.

## 7. TOTP-generointi

`src/totp/mod.rs`:

- Käytä `totp-rs`-cratea, algoritmi SHA1 (Google Authenticator -yhteensopivuus),
  6 numeroa, 30 sekunnin aikaikkuna — vastaa sitä, mitä Client-lukko odottaa.
- **Kellon lähde**: käytä palvelimen system-clockia, mutta lisää konfiguroitava
  `TOTP_SKEW_STEPS` (oletus 1) sallimaan ±1 aikaikkunan poikkeama, koska lukon
  RTC saattaa driftata — tämä tarkoittaa, että palautettu koodi tulee laskea
  _lukon_ näkökulmasta, ei suoraan tarkistaa palvelimella; palvelin vain generoi
  ja luottaa siihen, että lukko sallii saman skew-toleranssin.
- Palauta myös `valid_for_seconds` (jäljellä oleva aika nykyisessä
  aikaikkunassa), jotta käyttäjä näkee kuinka kauan koodi on voimassa.

## 8. Salaus levossa

`src/crypto.rs`:

- AES-256-GCM, avain `TOTP_ENCRYPTION_KEY` (32 tavua, base64) ympäristöstä.
- Jokaiselle secretille oma satunnainen nonce, tallennetaan nonce erikseen
  tietokantaan (ei koskaan uudelleenkäytä nonceä saman avaimen kanssa).
- Dev-ympäristössä avain voi tulla `.env`-tiedostosta; dokumentoi README:hen,
  että tuotannossa avain kuuluu vaulttiin/KMS:ään, ei ympäristömuuttujaan.

## 9. Rate limiting

- Aloita `tower_governor`-middlewarella per-`sub` (ei per-IP, koska sama
  käyttäjä voi tulla eri IP:stä) — vaatii custom key extractorin, joka lukee
  `sub`:n edellisen auth-middlewaren asettamasta `Extension`-arvosta.
- Oletusraja: esim. 6 pyyntöä / 5 min / käyttäjä (säädettävä env-muuttujalla),
  koska TOTP-koodin pyytäminen ei ole tiheä toiminto normaalikäytössä.
- Ylityksestä `429` + `request_log`-riville `rate_limited`.

## 10. Docker-testiympäristö

### `docker-compose.yml`

```yaml
services:
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_MULTIPLE_DATABASES: keycloak,totp
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: postgres
    volumes:
      - ./docker/postgres/init-multi-db.sh:/docker-entrypoint-initdb.d/init-multi-db.sh
    ports:
      - "5432:5432"
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 5s
      timeout: 5s
      retries: 10

  keycloak:
    image: quay.io/keycloak/keycloak:26.0
    command: ["start-dev", "--import-realm"]
    environment:
      KC_DB: postgres
      KC_DB_URL: jdbc:postgresql://postgres:5432/keycloak
      KC_DB_USERNAME: postgres
      KC_DB_PASSWORD: postgres
      KEYCLOAK_ADMIN: admin
      KEYCLOAK_ADMIN_PASSWORD: admin
    volumes:
      - ./docker/keycloak/realm-export.json:/opt/keycloak/data/import/realm-export.json
    ports:
      - "8080:8080"
    depends_on:
      postgres:
        condition: service_healthy
    healthcheck:
      test: ["CMD-SHELL", "curl -f http://localhost:8080/health/ready || exit 1"]
      interval: 10s
      timeout: 5s
      retries: 15

  totp-server:
    build: .
    environment:
      DATABASE_URL: postgres://postgres:postgres@postgres:5432/totp
      KEYCLOAK_ISSUER: http://keycloak:8080/realms/lock-demo
      TOTP_SERVER_KEYCLOAK_CLIENT_ID: totp-server
      TOTP_SERVER_KEYCLOAK_CLIENT_SECRET: dev-secret-change-me
      TOTP_ENCRYPTION_KEY: ${TOTP_ENCRYPTION_KEY:?set in .env}
      RUST_LOG: info,totp_server=debug
    ports:
      - "3000:3000"
    depends_on:
      keycloak:
        condition: service_healthy
      postgres:
        condition: service_healthy
```

### `docker/keycloak/realm-export.json`

Esikonfiguroi:

- Realm `lock-demo`
- Confidential client `totp-server` (service account,
  `client_credentials`-grantti käytöstä pois — tätä clientia käytetään vain
  introspektioon, ei tokenien hakuun), audience mapper joka lisää `totp-server`
  tokenin `aud`-claimiin sopivalle scope-nimelle (esim. `totp-access`)
- Public client `lock-cli` testikäyttäjän kirjautumiseen (`direct access grants`
  päällä, jotta `curl`/testit voivat hakea tokenin ilman selainta)
- Yksi testikäyttäjä (`testuser` / salasana ympäristömuuttujasta, ei
  kovakoodattuna)
- Client scope `totp-access`, mapattu `lock-cli`-clientin defaulttiscopeksi
  niin, että testuserin token sisältää oikean audiencen ilman erillistä pyyntöä

### `Dockerfile`

Multi-stage build: `rust:1.8x-slim` builder-vaiheeseen, kopioi binääri
`debian:bookworm-slim`-runtimeen, ei-root-käyttäjä, `HEALTHCHECK` osoittamaan
`/healthz`.

## 11. Testistrategia

1. **Yksikkötestit** (`tests/totp_generation.rs`): tarkista TOTP-arvo tunnetulla
   secretillä ja aikaleimalla RFC 6238 -testivektoreita vasten.
2. **Introspektio-mock** (`wiremock`): simuloi Keycloakin
   `/introspect`-vastaukset (`active: true/false`, virheelliset `aud`,
   verkkovirhe) ilman oikeaa Keycloak-konttia — nopea, deterministinen.
3. **Integraatiotestit** (`tests/introspection_integration.rs`,
   `testcontainers`): nosta oikea Keycloak-kontti (samalla realm-exportilla),
   hae oikea access token `lock-cli`-clientilla `testuser`-tunnuksilla, varmista
   päästä-päähän-kulku `/v1/totp/request`-endpointtiin asti.
4. **Manuaalinen end-to-end -skripti** (`scripts/e2e.sh`): `docker compose up`,
   hae token curlilla Keycloakilta, kutsu TOTP-endpointtia, tulosta PIN. Tämä on
   nopein tapa varmistaa koko ketju käsin kehityksen aikana.

## 12. Etenemisjärjestys Claude Codelle (askel askeleelta)

Jokainen askel tulisi committoida erikseen ja olla itsenäisesti
käännettävä/testattava.

1. **Scaffold**: `cargo new totp-server`, lisää riippuvuudet,
   `/healthz`-endpoint, `tracing`-alustus. Hyväksymiskriteeri: `cargo run`
   vastaa `/healthz`:iin.
2. **Docker-pohja**: kirjoita `docker-compose.yml` (Postgres + Keycloak +
   realm-export), ilman totp-serveriä vielä mukana. Hyväksymiskriteeri:
   `docker compose up` → Keycloak-admin-konsoli auki, realm `lock-demo`
   näkyvissä.
3. **Introspektioklientti + middleware** yksikkötestein (wiremock-pohjaiset
   testit ensin, toteutus sen jälkeen — TDD tälle osalle on erityisen
   hyödyllistä koska ulkoinen kontrakti on tarkasti tiedossa RFC:stä).
4. **Tietokanta + migraatiot**: `sqlx migrate`, `secret_store.rs`
   CRUD-toiminnot, salaus/purku roundtrip-testillä.
5. **`/v1/totp/provision`-endpoint** (admin-suojattu) + `/v1/totp/request`
   -endpoint yhdistettynä middlewareen ja secret_storeen.
6. **Rate limiting** -middleware ja sen testit (ylitä raja testissä, varmista
   429).
7. **Docker-integraatio täydeksi**: lisää `totp-server`-service
   `docker-compose.yml`:ään, `Dockerfile`, varmista `depends_on`/healthcheckit
   toimivat käynnistysjärjestyksen kanssa.
8. **Integraatiotestit testcontainersilla** oikeaa Keycloak-realmia vasten.
9. **`scripts/e2e.sh`** ja README, joka kuvaa: ympäristön pystytys,
   testitunnukset, esimerkki-curlit koko kululle (login → introspect tapahtuu
   palvelimen sisällä → PIN).
10. **Kovennus** (voidaan tehdä erillisenä seurantatehtävänä): audit-lokitus
    `request_log`-tauluun, `aud`-claimin pakollinen tarkistus, secrets
    tuotantoympäristössä vaultista, mTLS totp-serverin ja Keycloakin välillä.

## 13. Ympäristömuuttujat (`.env.example`)

```
DATABASE_URL=postgres://postgres:postgres@localhost:5432/totp
KEYCLOAK_ISSUER=http://localhost:8080/realms/lock-demo
TOTP_SERVER_KEYCLOAK_CLIENT_ID=totp-server
TOTP_SERVER_KEYCLOAK_CLIENT_SECRET=dev-secret-change-me
TOTP_ENCRYPTION_KEY=base64:REPLACE_ME_32_BYTES
TOTP_SKEW_STEPS=1
RATE_LIMIT_MAX_REQUESTS=6
RATE_LIMIT_WINDOW_SECONDS=300
RUST_LOG=info,totp_server=debug
```

## 14. Huomioita, jotka Claude Codelle kannattaa antaa ennalta

- Introspektio ei koskaan saa fail-openata verkkovirheessä.
- `aud`-claimin tarkistus on pakollinen, ei valinnainen — muuten mikä tahansa
  Keycloak-realmin token kelpaisi TOTP-pyyntöön.
- TOTP-secretit eivät koskaan päädy lokeihin, edes debug-tasolla.
- `provision`-endpoint palauttaa secretin selkokielisenä **vain kerran** —
  varmista ettei sitä koskaan lokiteta tai palauteta uudelleen.
