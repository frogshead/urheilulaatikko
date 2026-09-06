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

| Tuote | Hinta (arvio) | Lähde |
|---|---|---|
| Secukey XK1, IP66, Wiegand-näppäimistö | 130,90 € | Taloon.com / Netrauta.fi |
| Sähkösalpalukko (electric strike), 12V, ulkokäyttöön | 30–80 € | Sama kauppa tai K-Rauta |

**Kokonaisarvio ilman lukkoa:** ~60–90 €
**Kokonaisarvio lukon kanssa:** ~220–290 €

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
2. Poraa lukkomekanismin kiinnitysreiät (5mm) kannen alapintaan
   etureunan lähelle, hasp/salpalukon kiinnitystä varten
   (ks. malli, reikäväli 40mm).

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

1. Asenna hasp/salpalukon toinen puoli kannen alapinnan
   kiinnitysreikiin (vaihe 7).
2. Asenna vastakappale (Secukey XK1 + sähkösalpalukko) rungon
   etuseinään ulkopuolelle, kohtisuoraan hasp-kiinnityksen alle.
3. Vedä keypadin kaapeli takaseinän läpivientiholkin kautta
   sisälle elektroniikkakoteloon (valepohjan alle).

### Vaihe 11 — Viimeistely

1. Tarkista kaikki liimasaumat ja ruuvit vielä kertaalleen.
2. Käsittele mahdolliset käsittelemättömät kohdat maalilla/suoja-aineella.
3. Testaa vedenpoisto kaatamalla vähän vettä valepohjan päälle ja
   varmistamalla, että se valuu ulos etuseinän reiästä.

---

## 4. Huomioita

- Sarana ja lukko toimivat pinta-asennettuina — molemmat vahvistuslistat
  (rungossa ja kannessa) antavat riittävän tartuntapinnan ohuelle 15mm
  levylle.
- Valepohjan tulee olla irrotettavissa ilman työkaluja tai vain kevyellä
  voimalla, huoltoa varten (patterin vaihto, elektroniikan tarkastus).
- Jos säänkestävyys osoittautuu riittämättömäksi pelkällä tiivistenauhalla,
  harkitse pismauran jyrsimistä kannen alapintaan reunan tuntumaan
  jälkikäteen.
