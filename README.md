# urheilulaatikko - lock box

Urheiluvälinelaatikko seuroille, kunnille tai muille toimijoille. Laatikossa
voidaan säilyttää tavaroita kaikkien käyttöön siten, että antaa pääsy
valikoiduille henkilöille esimerkiksi seuran jäsenille. Tavoitteena integroida
nykyisiin palveluihin jos mahdollista esimerkiksi myclub, nimenhuuto tai
suomisportin -palvelun käyttäminen identititeetin hallintaan.

## Lukon toiminta

Lukon tulisi toimia ilman internet-yhteyttä. Toiminta perustuu vaihtuviin
TOTP-koodeihin ja salainen osuus on seurojen hallinnassa. Mikrokontrollerin tai
-prosesorin tulee pystyä tarkistamaan annettu koodi, mahdollisesti myös
edellinen koodi jos aikaikkuna kerennyt sulkeutua. Koodi voidaan välittää
lukolle käyttämällä mekaanista näppäimistöä, Bluetooth Low Energy (BLE)
komminikointia tai internetin yli jos mahdollista.

Alkuvaiheen toteutusalustaksi on valittu **ESP32-C3-DevKit-RUST-2**
(ESP32-C3-MINI-1-moduuli, RISC-V, 2,4 GHz Wi-Fi + Bluetooth 5 (LE), Li-Ion-akun
latauspiiri, useimmat I/O-pinnit vedetty header-riveille molemmin puolin).
Integroitu Wi-Fi ja BLE kattavat kaksi kolmesta yllä mainitusta
koodinvälitystavasta samalla laudalla. Ensimmäinen versio (Wiegand-näppäimistön
luku, TOTP-vertailu, releohjaus) on toteutettu `lock-client/`-hakemistossa —
ks. [lock-client/README.md](lock-client/README.md) yksityiskohdista ja
tunnetuista rajoituksista.

## Asiakassovellus

Mobiilisovellus, toiminnallisuus kuten vuokrasähköpotkulaudoissa.

## Hallintasovellus

Seuroille webbipohjainen hallintapaneeli, käyttäjien ja salaisuuksien hallintaa.
Sekä mahdollistten integraatioden toteuttamiseen.

## Repositorion rakenne

| Kansio          | Kuvaus |
| --------------- | ------ |
| `lock-client/`  | Lukon puolen logiikka, cargo-workspace: `core/` (laitteistoriippumaton TOTP/Wiegand/lukko-FSM, testattu `cargo test`illa) + `firmware/` (ESP32-C3-DevKit-RUST-2:lle käännettävä `no_std`-binääri, ks. `lock-client/README.md`) |
| `totp-server/`  | Palvelin, joka jakaa PIN-koodit Keycloak-autentikoinnin perusteella — ks. [totp-server/README.md](totp-server/README.md) |

Palvelimen käyttöönotto lyhyesti:

```bash
cd totp-server
cp .env.example .env
echo "TOTP_ENCRYPTION_KEY=base64:$(openssl rand -base64 32)" >> .env
docker compose up -d --build
./scripts/e2e.sh
```

`lock-client` ja `totp-server` laskevat koodin samoilla RFC 6238 -parametreilla
(SHA-1, 6 numeroa, 30 s ikkuna), joten sama secret tuottaa saman PIN-koodin
molemmissa päissä.

