# Lukittava filmivaneriarkku — ostoslista ja kokoamisohje

Ulkomitat 100 × 50 × 50 cm (leveys × syvyys × korkeus). Vastaa OpenSCAD-mallia
`lockable_plywood_box_v3.scad`.

---

## 1. Ostoslista

### Levymateriaali

| Tuote | Määrä | Käyttö |
|---|---|---|
| Vesivaneri 15mm, 1250×2500mm | 1 levy | Runko (4 seinää + pohja) + kansi |
| Vesivaneri 9mm | 1 pieni levy/reject-pala (~0,5m²) | Valepohja |

### Rimat

| Tuote | Määrä | Käyttö |
|---|---|---|
| Rima 20×20mm | ~3 m | 4× kulmalista + 2× valepohjan tukirima |
| Rima 20×30mm (tai lähin) | ~2 m | Saranan vahvistuslistat |

### Kiinnitystarvikkeet

| Tuote | Määrä |
|---|---|
| Ruostumaton pianosarana 1000mm TAI 3× arkkusarana ~100mm | 1 pakkaus |
| Ruuvit saranaan, RST 4×20mm | ~25 kpl |
| Ruuvit rimoihin, RST 4×40mm | ~50 kpl |
| PU-liima (esim. Sikaflex/Casco Strong) | 1 tuubi |

### Läpiviennit ja tiivistys

| Tuote | Määrä |
|---|---|
| Kaapeliholkki, IP-luokiteltu, ~12mm | 1 kpl |
| Kumitulppa/suodatinverkko 10mm (valinnainen) | 1 kpl |
| EPDM-tiivistenauha, itsekiinnittyvä | ~3,5 m |

### Pintakäsittely

| Tuote | Määrä |
|---|---|
| Akryylimaali tai puunsuoja-aine | 1 tölkki |

### Lukkomekanismi

Tilattu ja saapunut (AliExpress), hinnat tilausvahvistuksesta:

| Tuote | Malli | Tekniset tiedot |
|---|---|---|
| RFID + koodinäppäimistö (standalone access-kontrolleri) | LUCKING DOOR S601/S602 | **IP68**, sinkkiseos-kotelo (vandaalinkestävä), 125kHz EM- ja 13,56MHz Mifare-kortit, kortti/PIN/kortti+PIN, Wiegand 26 sisään- ja ulostulo, DC 12V, virrankulutus ~30mA, käyttäjäkapasiteetti 2000, lukon rele max 3A (oikosulkusuojattu), hälytysrele max 20A, ovirele säädettävä 0–99s, hälytysaika säädettävä 0–3min, taustavalaistu näppäimistö, summeri, käyttölämpötila −45…60 °C, koko 120×76×20mm |
| Electric drop bolt -lukko (upotettava, moottoroitu salpa) | Asia-Teco B6L-NC / B7L-NC, **fail-secure (NC)** | DC 12V, käynnistysvirta 1,1A / pitovirta 0,14A, viive säädettävä 0/3/6s, palautesignaali (feedback), alumiiniseos, 200×28×38mm |

Lukon **NC-versio on fail-secure**: salpa pysyy lukossa ilman virtaa ja avautuu
vain kun ohjaimelta tulee virta — säilytyslaatikkoon oikea valinta, koska
sähkökatko ei itsestään avaa lukkoa. Kontrollerin lukon relelähtö (max 3A)
kattaa salvan käynnistysvirran (1,1A) reilusti, joten erillistä
apurelettä ei tarvita. IP68-luokitus tekee näppäimistöstä sellaisenaan
säänkestävän ulkoasennukseen, joten erillistä sadesuojaa ei välttämättä
tarvita (varmista silti asennuksessa ettei vesi pääse valumaan suoraan
kaapelin läpivientiin päin).

**Huomio arkkitehtuuriin:** S601/S602 on *standalone*-kontrolleri, joka
tarkistaa kortin/PIN-koodin omasta 2000 käyttäjän muististaan ja laukaisee
relettä itse — se ei tiedä mitään TOTP:sta. Repon TOTP-pohjaisen,
ajassa vaihtuvan koodin käyttämiseksi kaksi vaihtoehtoa:

1. Käytetään näppäimistöä pelkkänä **Wiegand-ulostulolla varustettuna
   lukijana/näppäimistönä** (ei standalone-tilassa), ja Wiegand-data luetaan
   `lock-client`:ia ajavalla mikro-ohjaimella, joka laskee TOTP:n ja ohjaa
   relettä itse GPIO:lta — tätä varten `lock-client`iin pitää vielä
   toteuttaa Wiegand-luku ja releohjaus (ei vielä tehty).
2. Käytetään laitetta standalone-tilassa kiinteällä/hallinnollisesti
   vaihdettavalla PIN-koodilla ilman TOTP:a — yksinkertaisempi, mutta
   poikkeaa README:n kuvaamasta TOTP-suunnitelmasta.

**Kokonaisarvio muu materiaali (ilman lukkoa):** ~60–90 €
Lukko ja näppäimistö on jo hankittu erikseen, hinta ei sisälly yllä olevaan
materiaaliarvioon.

---

## 2. Leikkuulista (mittasahauspalvelua varten)

### 15mm vesivaneri

| Osa | Kpl | Mitat (mm) |
|---|---|---|
| Pohja | 1 | 1000 × 500 |
| Sivuseinä | 2 | 500 × 500 |
| Etu-/takaseinä | 2 | 970 × 500 |
| Kansi | 1 | 1040 × 540 |

### 9mm vesivaneri

| Osa | Kpl | Mitat (mm) |
|---|---|---|
| Valepohja | 1 | 970 × 470 (sovitetaan asennuksessa) |

### Rimat

| Osa | Kpl | Poikkileikkaus (mm) | Pituus (mm) |
|---|---|---|---|
| Kulmalista | 4 | 20 × 20 | 485 |
| Valepohjan tukirima | 2 | 20 × 20 | 462 |
| Saranan vahvistuslista, runko | 1 | 18 × 30 | 970 |
| Saranan vahvistuslista, kansi | 1 | 18 × 30 | 1000 |

Mitat ovat lopullisia, eivät sisällä sahausvaraa — kerro palveluntarjoajalle jos
he tarvitsevat +1–2mm/reuna.

---

## 3. Kokoamisohje

### Vaihe 1 — Reunojen käsittely ennen kasausta

Maalaa/käsittele kaikki sahatut reunat akryylimaalilla tai puunsuoja-aineella
**ennen** kasausta. Filmipinnoite suojaa levyn tasopinnat, mutta leikatut
reunat imevät kosteutta ellei niitä käsitellä.

### Vaihe 2 — Kulmalistojen kiinnitys

1. Liimaa (PU-liima) ja ruuvaa 4 kulmalistaa (20×20×485mm) pohjan neljään
   nurkkaan pystyasentoon, kiinni siihen seinään johon ne luontevimmin sopivat.
2. Anna liiman kuivua ennen seuraavaa vaihetta (tarkista liiman
   kuivumisaika pakkauksesta, tyypillisesti useita tunteja).

### Vaihe 3 — Rungon kasaus

1. Liimaa ja ruuvaa pohja (1000×500mm) kiinni sivuseiniin (500×500mm ×2)
   — sivuseinät tulevat pohjan lyhyille sivuille (500mm reunalle).
2. Liimaa ja ruuvaa etu-/takaseinät (970×500mm ×2) paikoilleen sivuseinien
   väliin, kiinni sekä pohjaan että kulmalistoihin.
3. Varmista suorakulmaisuus mittaamalla diagonaalit nurkasta nurkkaan —
   molempien diagonaalien tulee olla yhtä pitkät.
4. Ruuvaa vielä lisäruuvit kulmalistojen läpi seiniin, kun runko on suorassa.

### Vaihe 4 — Saranan vahvistuslistan kiinnitys

1. Liimaa ja ruuvaa saranan vahvistuslista (18×30×970mm) rungon
   **takaseinän sisäpintaan**, yläreunan tuntumaan (ks. malli:
   `hinge_batten_box`).
2. Anna kuivua ennen saranan asennusta.

### Vaihe 5 — Valepohjan tukirimat ja valepohja

1. Merkitse sivuseinien sisäpintoihin korkeus, jossa tukirimat tulevat
   (kalteva linja — matalampi etureunassa, korkeampi takareunassa, ks. malli
   `FLOOR_SLOPE`).
2. Liimaa ja ruuvaa tukirimat (20×20×462mm) molempiin sivuseiniin
   merkittyä kaltevaa linjaa pitkin.
3. Sovita valepohja (970×470mm, 9mm) tukirimojen päälle — **älä liimaa
   kiinni**, sen tulee olla irrotettavissa huoltoa varten. Lyhennä
   tarvittaessa sovituksessa.
4. Poraa vedenpoistoreikä (10mm) etuseinän läpi valepohjan matalimman
   kohdan tasolle, ennen valepohjan asennusta paikoilleen (helpompi porata
   kun pääsee käsiksi molemmin puolin).

### Vaihe 6 — Kaapelin läpivienti

1. Poraa reikä (12mm) takaseinään, lähelle yläreunaa/saranaa, keypadin
   kaapelia varten.
2. Asenna IP-luokiteltu kaapeliholkki reikään ennen kaapelointia — tämä
   estää veden pääsyn kaapelin ympäriltä.

### Vaihe 7 — Kannen valmistelu

1. Liimaa ja ruuvaa saranan vahvistuslista (18×30×1000mm) kannen
   **alapintaan**, takareunan kohdalle (ks. malli `flat_lid`).
2. Poraa lukkomekanismin kiinnitysreiät kannen alapintaan etureunan
   lähelle Asia-Teco-salvan (200×28×38mm) rungon mukaan. **Mittaa reikäväli
   suoraan salvan mukana tulevasta asennuspohjasta/-ohjeesta** — mallin
   40mm-oletus oli laskettu geneeriselle hasp-lukolle, ei tälle
   drop bolt -mallille.

### Vaihe 8 — Tiivistys

1. Liimaa EPDM-tiivistenauha rungon yläreunan kiertävälle pinnalle,
   koko matkalle — tämä on erityisen tärkeä nyt kun kansi on tasainen
   levy eikä "hattu"-mallia, koska sauma ei ole muuten yhtä tiivis.

### Vaihe 9 — Saranan asennus

1. Aseta kansi paikoilleen rungon päälle, tarkista että ylitys
   (20mm/sivu) on tasainen joka puolella.
2. Merkitse saranan paikat: toinen leveys rungon takaseinän
   ulkopintaan/yläreunaan (vahvistuslista sisäpuolella tukena),
   toinen kannen alapinnan takareunaan (vahvistuslista tukena).
3. Ruuvaa sarana kiinni molempiin osiin RST 4×20mm ruuveilla.
4. Testaa avautuminen/sulkeutuminen ennen lukkomekanismin asennusta.

### Vaihe 10 — Lukkomekanismin asennus

1. Asenna Asia-Teco-drop bolt -lukko kannen alapinnan kiinnitysreikiin
   (vaihe 7).
2. Poraa/jyrsi vastakappaleeksi reikä rungon etuseinän yläreunaan,
   kohtisuoraan salvan pultin alle, niin että pultti työntyy siihen kannen
   ollessa kiinni. Tarkista pultin heittopituus ja -suunta salvan
   asennusohjeesta ennen poraamista.
3. Asenna LUCKING DOOR S601/S602 -näppäimistö rungon etuseinään
   ulkopuolelle. **Testaa lukon toiminta virtalähteen kanssa ennen kuin
   suljet kantta** — salpa on fail-secure (NC), joten väärin kytkettynä tai
   ilman virtaa se pysyy lukossa eikä laatikkoon pääse enää käsiksi.
4. Vedä näppäimistön kaapeli takaseinän läpivientiholkin kautta sisälle
   elektroniikkakoteloon (valepohjan alle), ja kytke salvan johdot
   näppäimistön lukon relelähtöön (ks. laitteiden omat kytkentäkaaviot).

### Vaihe 11 — Viimeistely

1. Tarkista kaikki liimasaumat ja ruuvit vielä kertaalleen.
2. Käsittele mahdolliset käsittelemättömät kohdat maalilla/suoja-aineella.
3. Testaa vedenpoisto kaatamalla vähän vettä valepohjan päälle ja
   varmistamalla, että se valuu ulos etuseinän reiästä.

---

## 4. Huomioita

- Sarana toimii pinta-asennettuna, vahvistuslista antaa riittävän
  tartuntapinnan ohuelle 15mm levylle. Asia-Teco-salpa sen sijaan on
  upotettava/moottoroitu drop bolt -lukko, ei pinta-asenteinen hasp —
  vastakappaleeksi tarvitaan reikä rungon etuseinään (ks. vaihe 10),
  ei pelkkää ruuvattavaa metallilenkkiä.
- Salpa on tilattu **fail-secure (NC)** -versiona: se pysyy lukossa ilman
  virtaa. Muista tämä sekä asennuksen testauksessa (älä jää lukkojen taakse
  ilman virtalähdettä kytkettynä) että virransyötön suunnittelussa —
  akun tyhjeneminen tarkoittaa laatikon jäämistä kiinni, ei auki.
- LUCKING DOOR S601/S602 on IP68-luokiteltu standalone-kontrolleri, joten se
  kestää ulkoasennuksen sellaisenaan. Jos koodintarkistus halutaan tehdä
  repon TOTP-logiikalla eikä laitteen omalla staattisella
  kortti/PIN-muistilla, näppäimistö pitää käyttää Wiegand-lukijana ja
  TOTP-vertailu + releohjaus toteuttaa erikseen `lock-client`-puolella —
  tätä ei ole vielä koodattu.
- Valepohjan tulee olla irrotettavissa ilman työkaluja tai vain kevyellä
  voimalla, huoltoa varten (patterin vaihto, elektroniikan tarkastus).
- Jos säänkestävyys osoittautuu riittämättömäksi pelkällä tiivistenauhalla,
  harkitse pismauran jyrsimistä kannen alapintaan reunan tuntumaan
  jälkikäteen.
