// ============================================================
// Lukittava filmivaneriarkku — parametrinen malli (v3)
// ============================================================
// Ulkomitat 100 x 50 x 50 cm (leveys x syvyys x korkeus).
// KORKEUS ON OLETUS — vahvista jos tarkoitit muuta.
//
// v3: kansi yksinkertaistettu YHDEKSI TASAISEKSI LEVYKSI, joka
// lepää runko-reunojen päällä ja ylittää ne LID_OVERHANG mm
// kaikilta sivuilta. Ei enää erillistä "hattu"-skirttiä.
//
// Sarana: koska levy on ohut (15mm), lisätty paikallinen
// vahvistuslista kannen ALAPINTAAN takareunan kohdalle —
// vain siihen kohtaan, ei koko kannen ympäri. Tämä antaa
// saranaruuveille tartuntapintaa ilman että kansi muuttuu
// "hattu"-rakenteeksi.
//
// Yksiköt: mm
// ============================================================

// ---------- PÄÄMITAT ----------
WIDTH   = 1000;
DEPTH   = 500;
HEIGHT  = 500;   // OLETUS — tarkista
T       = 15;    // rungon/kannen vesivanerin paksuus

// ---------- KANSI ----------
LID_T        = T;     // kannen levyn paksuus
LID_OVERHANG = 20;     // kuinka paljon kansi ylittää rungon reunat, per sivu

// ---------- VALEPOHJA ----------
FLOOR_T      = 9;
CAVITY_H_LOW = 70;
FLOOR_SLOPE  = 15;
FLOOR_MARGIN = 4;
BATTEN_SEC   = 20;   // valepohjan tukirimat

// ---------- KULMALISTAT (runko) ----------
CORNER_BATTEN_SEC = 20;

// ---------- SARANAN VAHVISTUS ----------
HINGE_BATTEN_W = 30;   // pituus kansi/runko-suunnassa (y), mm
HINGE_BATTEN_T = 18;   // korkeus/paksuus (z), mm — kuinka paljon lisää materiaalia

// ---------- VEDENPOISTO ----------
DRAIN_D = 10;

// ---------- LUKKOMEKANISMI (etureunassa) ----------
LOCK_HOLE_D = 5;
LOCK_HOLE_SPACING = 40;

// ---------- KAAPELIN LÄPIVIENTI ----------
CABLE_HOLE_D = 12;

$fn = 48;

// ============================================================
// MODUULIT
// ============================================================

module box_shell(w, d, h, t) {
    difference() {
        cube([w, d, h]);
        translate([t, t, t])
            cube([w - 2*t, d - 2*t, h]);
    }
}

module corner_battens(w, d, h, t, sec) {
    positions = [
        [t, t],
        [w - t - sec, t],
        [t, d - t - sec],
        [w - t - sec, d - t - sec]
    ];
    color("saddlebrown")
    for (p = positions)
        translate([p[0], p[1], t])
            cube([sec, sec, h - t]);
}

// Vahvistuslista rungon TAKASEINÄN sisäpintaan, saranaruuveja varten
module hinge_batten_box(w, d, h, t, batten_w, batten_t) {
    color("saddlebrown")
    translate([t, d - t - batten_t, h - batten_w])
        cube([w - 2*t, batten_t, batten_w]);
}

// Yksinkertainen ylimenevä kansi: tasainen levy + paikallinen
// vahvistuslista alapinnassa takareunan kohdalla
module flat_lid(w, d, t, overhang, batten_w, batten_t) {
    lid_w = w + 2*overhang;
    lid_d = d + 2*overhang;

    color("tan", 0.85)
    cube([lid_w, lid_d, t]);

    // Vahvistuslista alapintaan, takareunan kohdalle (sarana tähän)
    color("saddlebrown")
    translate([overhang, lid_d - overhang - batten_t, -batten_w])
        cube([w, batten_t, batten_w]);

    // Lukkomekanismin kiinnitysreiät etureunan alapintaan
    // (hasp/salpalukko ruuvataan tähän, vastakappale runkoon)
    cx = lid_w / 2;
    for (dx = [-LOCK_HOLE_SPACING/2, LOCK_HOLE_SPACING/2])
        translate([cx + dx, overhang * 0.6, -1])
            color("black")
            cylinder(d = LOCK_HOLE_D, h = t + 2);
}

module false_floor(inner_w, inner_d, base_z) {
    fw = inner_w - 2*FLOOR_MARGIN;
    fd = inner_d - 2*FLOOR_MARGIN;
    angle = atan2(FLOOR_SLOPE, fd);

    color("burlywood")
    intersection() {
        translate([T + FLOOR_MARGIN, T + FLOOR_MARGIN, base_z])
        translate([0, 0, FLOOR_SLOPE/2])
        translate([fw/2, fd/2, 0])
        rotate([angle, 0, 0])
        translate([-fw/2, -fd/2, -FLOOR_T/2])
            cube([fw, fd, FLOOR_T]);

        translate([T, T, base_z - 5])
            cube([inner_w, inner_d, FLOOR_SLOPE + FLOOR_T + 10]);
    }

    for (side = [T + 2, T + inner_w - BATTEN_SEC - 2])
        color("sienna")
        translate([side, T + FLOOR_MARGIN, base_z - BATTEN_SEC])
        translate([0, fd/2, 0])
        rotate([angle, 0, 0])
        translate([0, -fd/2, 0])
            cube([BATTEN_SEC, fd, BATTEN_SEC]);
}

// ============================================================
// KOKOONPANO (esikatselu)
// ============================================================

inner_w = WIDTH - 2*T;
inner_d = DEPTH - 2*T;

color("wheat", 0.9)
    box_shell(WIDTH, DEPTH, HEIGHT, T);

corner_battens(WIDTH, DEPTH, HEIGHT, T, CORNER_BATTEN_SEC);
hinge_batten_box(WIDTH, DEPTH, HEIGHT, T, HINGE_BATTEN_W, HINGE_BATTEN_T);

false_floor(inner_w, inner_d, T + CAVITY_H_LOW);

translate([WIDTH/2, T + 1, T + CAVITY_H_LOW + FLOOR_T/2])
    rotate([-90, 0, 0])
    color("black")
    cylinder(d = DRAIN_D, h = T + 2);

translate([WIDTH - 100, DEPTH - T - 1, HEIGHT - 40])
    rotate([-90, 0, 0])
    color("black")
    cylinder(d = CABLE_HOLE_D, h = T + 2);

// Kansi levätä suoraan rungon reunojen päällä
translate([-LID_OVERHANG, -LID_OVERHANG, HEIGHT])
    flat_lid(WIDTH, DEPTH, LID_T, LID_OVERHANG, HINGE_BATTEN_W, HINGE_BATTEN_T);

// ============================================================
// SARANA — pinta-asennus, paikallinen vahvistus
// ============================================================
// Kansi on tasainen levy, ei skirtiä. Sarana asennetaan
// PINTA-ASENNUKSENA runko/kansi-sauman päälle:
//   - toinen sivu ruuvataan rungon takaseinän YLÄREUNAAN/-pintaan
//     (hinge_batten_box antaa tukea sisäpuolelta)
//   - toinen sivu ruuvataan kannen ALAPINTAAN takareunan kohdalle
//     (flat_lid:n paikallinen vahvistuslista antaa tukea)
// Koska kansi ei ylety reunojen ohi pystysuoraan, sarana näkyy
// päältä katsottuna saumassa — tämä on normaalia arkku-tyylisissä
// laatikoissa (esim. K-Raudan piha-arkut).
//
// HUOM vedenpitävyydestä: koska kansi on tasainen levy jonka
// alapinta on suoraan rungon reunan päällä, sauma EI ole yhtä
// tiivis kuin skirt-mallissa. Jos säänkestävyys on tärkeä,
// harkitse tiivistenauhaa (esim. EPDM-vaahtonauha) rungon
// yläreunaan kannen alapuolelle, tai pisarauran jyrsimistä
// kannen alapintaan reunan tuntumaan.

echo(str("Pohja: ", WIDTH, " x ", DEPTH, " mm, t=", T));
echo(str("Etu-/takaseinä: ", WIDTH - 2*T, " x ", HEIGHT, " mm, t=", T));
echo(str("Sivuseinä: ", DEPTH, " x ", HEIGHT, " mm, t=", T));
echo(str("Kansi: ", WIDTH + 2*LID_OVERHANG, " x ", DEPTH + 2*LID_OVERHANG,
         " mm, t=", LID_T));
echo(str("Valepohja: ", inner_w - 2*FLOOR_MARGIN, " x ",
         inner_d - 2*FLOOR_MARGIN, " mm, t=", FLOOR_T,
         " (kaltevuus ", FLOOR_SLOPE, " mm)"));
echo(str("Kulmalista (4 kpl): ", CORNER_BATTEN_SEC, " x ", CORNER_BATTEN_SEC,
         " x ", HEIGHT - T, " mm"));
echo(str("Saranan vahvistuslista, runko: ", WIDTH - 2*T, " x ",
         HINGE_BATTEN_T, " x ", HINGE_BATTEN_W, " mm"));
echo(str("Saranan vahvistuslista, kansi: ", WIDTH, " x ",
         HINGE_BATTEN_T, " x ", HINGE_BATTEN_W, " mm"));
