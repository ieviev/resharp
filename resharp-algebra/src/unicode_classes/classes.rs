use crate::{NodeId, RegexBuilder};

pub fn build_word_class(b: &mut RegexBuilder) -> NodeId {
    let n0 = b.mk_ranges_u8(&[(0x30, 0x39), (0x41, 0x5A), (0x5F, 0x5F), (0x61, 0x7A)]);
    let n1 = b.mk_range_u8(0xC2, 0xC2);
    let n2 = b.mk_ranges_u8(&[(0xAA, 0xAA), (0xB5, 0xB5), (0xBA, 0xBA)]);
    let n3 = b.mk_concat(n1, n2);
    let n4 = b.mk_range_u8(0xC3, 0xC3);
    let n5 = b.mk_ranges_u8(&[(0x80, 0x96), (0x98, 0xB6), (0xB8, 0xBF)]);
    let n6 = b.mk_concat(n4, n5);
    let n7 = b.mk_range_u8(0xC4, 0xCA);
    let n8 = b.mk_range_u8(0x80, 0xBF);
    let n9 = b.mk_concat(n7, n8);
    let n10 = b.mk_range_u8(0xCB, 0xCB);
    let n11 = b.mk_ranges_u8(&[
        (0x80, 0x81),
        (0x86, 0x91),
        (0xA0, 0xA4),
        (0xAC, 0xAC),
        (0xAE, 0xAE),
    ]);
    let n12 = b.mk_concat(n10, n11);
    let n13 = b.mk_range_u8(0xCC, 0xCC);
    let n14 = b.mk_concat(n13, n8);
    let n15 = b.mk_range_u8(0xCD, 0xCD);
    let n16 = b.mk_ranges_u8(&[(0x80, 0xB4), (0xB6, 0xB7), (0xBA, 0xBD), (0xBF, 0xBF)]);
    let n17 = b.mk_concat(n15, n16);
    let n18 = b.mk_range_u8(0xCE, 0xCE);
    let n19 = b.mk_ranges_u8(&[
        (0x86, 0x86),
        (0x88, 0x8A),
        (0x8C, 0x8C),
        (0x8E, 0xA1),
        (0xA3, 0xBF),
    ]);
    let n20 = b.mk_concat(n18, n19);
    let n21 = b.mk_range_u8(0xCF, 0xCF);
    let n22 = b.mk_ranges_u8(&[(0x80, 0xB5), (0xB7, 0xBF)]);
    let n23 = b.mk_concat(n21, n22);
    let n24 = b.mk_range_u8(0xD0, 0xD1);
    let n25 = b.mk_concat(n24, n8);
    let n26 = b.mk_range_u8(0xD2, 0xD2);
    let n27 = b.mk_ranges_u8(&[(0x80, 0x81), (0x83, 0xBF)]);
    let n28 = b.mk_concat(n26, n27);
    let n29 = b.mk_range_u8(0xD3, 0xD3);
    let n30 = b.mk_concat(n29, n8);
    let n31 = b.mk_range_u8(0xD4, 0xD4);
    let n32 = b.mk_ranges_u8(&[(0x80, 0xAF), (0xB1, 0xBF)]);
    let n33 = b.mk_concat(n31, n32);
    let n34 = b.mk_range_u8(0xD5, 0xD5);
    let n35 = b.mk_ranges_u8(&[(0x80, 0x96), (0x99, 0x99), (0xA0, 0xBF)]);
    let n36 = b.mk_concat(n34, n35);
    let n37 = b.mk_range_u8(0xD6, 0xD6);
    let n38 = b.mk_ranges_u8(&[(0x80, 0x88), (0x91, 0xBD), (0xBF, 0xBF)]);
    let n39 = b.mk_concat(n37, n38);
    let n40 = b.mk_range_u8(0xD7, 0xD7);
    let n41 = b.mk_ranges_u8(&[
        (0x81, 0x82),
        (0x84, 0x85),
        (0x87, 0x87),
        (0x90, 0xAA),
        (0xAF, 0xB2),
    ]);
    let n42 = b.mk_concat(n40, n41);
    let n43 = b.mk_range_u8(0xD8, 0xD8);
    let n44 = b.mk_ranges_u8(&[(0x90, 0x9A), (0xA0, 0xBF)]);
    let n45 = b.mk_concat(n43, n44);
    let n46 = b.mk_range_u8(0xD9, 0xD9);
    let n47 = b.mk_ranges_u8(&[(0x80, 0xA9), (0xAE, 0xBF)]);
    let n48 = b.mk_concat(n46, n47);
    let n49 = b.mk_range_u8(0xDA, 0xDA);
    let n50 = b.mk_concat(n49, n8);
    let n51 = b.mk_range_u8(0xDB, 0xDB);
    let n52 = b.mk_ranges_u8(&[
        (0x80, 0x93),
        (0x95, 0x9C),
        (0x9F, 0xA8),
        (0xAA, 0xBC),
        (0xBF, 0xBF),
    ]);
    let n53 = b.mk_concat(n51, n52);
    let n54 = b.mk_range_u8(0xDC, 0xDC);
    let n55 = b.mk_range_u8(0x90, 0xBF);
    let n56 = b.mk_concat(n54, n55);
    let n57 = b.mk_range_u8(0xDD, 0xDD);
    let n58 = b.mk_ranges_u8(&[(0x80, 0x8A), (0x8D, 0xBF)]);
    let n59 = b.mk_concat(n57, n58);
    let n60 = b.mk_range_u8(0xDE, 0xDE);
    let n61 = b.mk_range_u8(0x80, 0xB1);
    let n62 = b.mk_concat(n60, n61);
    let n63 = b.mk_range_u8(0xDF, 0xDF);
    let n64 = b.mk_ranges_u8(&[(0x80, 0xB5), (0xBA, 0xBA), (0xBD, 0xBD)]);
    let n65 = b.mk_concat(n63, n64);
    let n66 = b.mk_union(n62, n65);
    let n67 = b.mk_union(n59, n66);
    let n68 = b.mk_union(n56, n67);
    let n69 = b.mk_union(n53, n68);
    let n70 = b.mk_union(n50, n69);
    let n71 = b.mk_union(n48, n70);
    let n72 = b.mk_union(n45, n71);
    let n73 = b.mk_union(n42, n72);
    let n74 = b.mk_union(n39, n73);
    let n75 = b.mk_union(n36, n74);
    let n76 = b.mk_union(n33, n75);
    let n77 = b.mk_union(n30, n76);
    let n78 = b.mk_union(n28, n77);
    let n79 = b.mk_union(n25, n78);
    let n80 = b.mk_union(n23, n79);
    let n81 = b.mk_union(n20, n80);
    let n82 = b.mk_union(n17, n81);
    let n83 = b.mk_union(n14, n82);
    let n84 = b.mk_union(n12, n83);
    let n85 = b.mk_union(n9, n84);
    let n86 = b.mk_union(n6, n85);
    let n87 = b.mk_union(n3, n86);
    b.mk_union(n0, n87)
}

pub fn build_digit_class(b: &mut RegexBuilder) -> NodeId {
    let n0 = b.mk_range_u8(0x30, 0x39);
    let n1 = b.mk_range_u8(0xD9, 0xD9);
    let n2 = b.mk_range_u8(0xA0, 0xA9);
    let n3 = b.mk_concat(n1, n2);
    let n4 = b.mk_range_u8(0xDB, 0xDB);
    let n5 = b.mk_range_u8(0xB0, 0xB9);
    let n6 = b.mk_concat(n4, n5);
    let n7 = b.mk_range_u8(0xDF, 0xDF);
    let n8 = b.mk_range_u8(0x80, 0x89);
    let n9 = b.mk_concat(n7, n8);
    let n10 = b.mk_union(n6, n9);
    let n11 = b.mk_union(n3, n10);
    b.mk_union(n0, n11)
}

pub fn build_space_class(b: &mut RegexBuilder) -> NodeId {
    let n0 = b.mk_ranges_u8(&[(0x09, 0x0D), (0x20, 0x20)]);
    let n1 = b.mk_range_u8(0xC2, 0xC2);
    let n2 = b.mk_ranges_u8(&[(0x85, 0x85), (0xA0, 0xA0)]);
    let n3 = b.mk_concat(n1, n2);
    b.mk_union(n0, n3)
}

pub fn build_word_class_full(b: &mut RegexBuilder) -> NodeId {
    let n0 = b.mk_ranges_u8(&[(0x30, 0x39), (0x41, 0x5A), (0x5F, 0x5F), (0x61, 0x7A)]);
    let n1 = b.mk_range_u8(0xC2, 0xC2);
    let n2 = b.mk_ranges_u8(&[(0xAA, 0xAA), (0xB5, 0xB5), (0xBA, 0xBA)]);
    let n3 = b.mk_concat(n1, n2);
    let n4 = b.mk_range_u8(0xC3, 0xC3);
    let n5 = b.mk_ranges_u8(&[(0x80, 0x96), (0x98, 0xB6), (0xB8, 0xBF)]);
    let n6 = b.mk_concat(n4, n5);
    let n7 = b.mk_range_u8(0xC4, 0xCA);
    let n8 = b.mk_range_u8(0x80, 0xBF);
    let n9 = b.mk_concat(n7, n8);
    let n10 = b.mk_range_u8(0xCB, 0xCB);
    let n11 = b.mk_ranges_u8(&[
        (0x80, 0x81),
        (0x86, 0x91),
        (0xA0, 0xA4),
        (0xAC, 0xAC),
        (0xAE, 0xAE),
    ]);
    let n12 = b.mk_concat(n10, n11);
    let n13 = b.mk_range_u8(0xCC, 0xCC);
    let n14 = b.mk_concat(n13, n8);
    let n15 = b.mk_range_u8(0xCD, 0xCD);
    let n16 = b.mk_ranges_u8(&[(0x80, 0xB4), (0xB6, 0xB7), (0xBA, 0xBD), (0xBF, 0xBF)]);
    let n17 = b.mk_concat(n15, n16);
    let n18 = b.mk_range_u8(0xCE, 0xCE);
    let n19 = b.mk_ranges_u8(&[
        (0x86, 0x86),
        (0x88, 0x8A),
        (0x8C, 0x8C),
        (0x8E, 0xA1),
        (0xA3, 0xBF),
    ]);
    let n20 = b.mk_concat(n18, n19);
    let n21 = b.mk_range_u8(0xCF, 0xCF);
    let n22 = b.mk_ranges_u8(&[(0x80, 0xB5), (0xB7, 0xBF)]);
    let n23 = b.mk_concat(n21, n22);
    let n24 = b.mk_range_u8(0xD0, 0xD1);
    let n25 = b.mk_concat(n24, n8);
    let n26 = b.mk_range_u8(0xD2, 0xD2);
    let n27 = b.mk_ranges_u8(&[(0x80, 0x81), (0x83, 0xBF)]);
    let n28 = b.mk_concat(n26, n27);
    let n29 = b.mk_range_u8(0xD3, 0xD3);
    let n30 = b.mk_concat(n29, n8);
    let n31 = b.mk_range_u8(0xD4, 0xD4);
    let n32 = b.mk_ranges_u8(&[(0x80, 0xAF), (0xB1, 0xBF)]);
    let n33 = b.mk_concat(n31, n32);
    let n34 = b.mk_range_u8(0xD5, 0xD5);
    let n35 = b.mk_ranges_u8(&[(0x80, 0x96), (0x99, 0x99), (0xA0, 0xBF)]);
    let n36 = b.mk_concat(n34, n35);
    let n37 = b.mk_range_u8(0xD6, 0xD6);
    let n38 = b.mk_ranges_u8(&[(0x80, 0x88), (0x91, 0xBD), (0xBF, 0xBF)]);
    let n39 = b.mk_concat(n37, n38);
    let n40 = b.mk_range_u8(0xD7, 0xD7);
    let n41 = b.mk_ranges_u8(&[
        (0x81, 0x82),
        (0x84, 0x85),
        (0x87, 0x87),
        (0x90, 0xAA),
        (0xAF, 0xB2),
    ]);
    let n42 = b.mk_concat(n40, n41);
    let n43 = b.mk_range_u8(0xD8, 0xD8);
    let n44 = b.mk_ranges_u8(&[(0x90, 0x9A), (0xA0, 0xBF)]);
    let n45 = b.mk_concat(n43, n44);
    let n46 = b.mk_range_u8(0xD9, 0xD9);
    let n47 = b.mk_ranges_u8(&[(0x80, 0xA9), (0xAE, 0xBF)]);
    let n48 = b.mk_concat(n46, n47);
    let n49 = b.mk_range_u8(0xDA, 0xDA);
    let n50 = b.mk_concat(n49, n8);
    let n51 = b.mk_range_u8(0xDB, 0xDB);
    let n52 = b.mk_ranges_u8(&[
        (0x80, 0x93),
        (0x95, 0x9C),
        (0x9F, 0xA8),
        (0xAA, 0xBC),
        (0xBF, 0xBF),
    ]);
    let n53 = b.mk_concat(n51, n52);
    let n54 = b.mk_range_u8(0xDC, 0xDC);
    let n55 = b.mk_range_u8(0x90, 0xBF);
    let n56 = b.mk_concat(n54, n55);
    let n57 = b.mk_range_u8(0xDD, 0xDD);
    let n58 = b.mk_ranges_u8(&[(0x80, 0x8A), (0x8D, 0xBF)]);
    let n59 = b.mk_concat(n57, n58);
    let n60 = b.mk_range_u8(0xDE, 0xDE);
    let n61 = b.mk_range_u8(0x80, 0xB1);
    let n62 = b.mk_concat(n60, n61);
    let n63 = b.mk_range_u8(0xDF, 0xDF);
    let n64 = b.mk_ranges_u8(&[(0x80, 0xB5), (0xBA, 0xBA), (0xBD, 0xBD)]);
    let n65 = b.mk_concat(n63, n64);
    let n66 = b.mk_range_u8(0xE0, 0xE0);
    let n67 = b.mk_range_u8(0xA0, 0xA0);
    let n68 = b.mk_range_u8(0x80, 0xAD);
    let n69 = b.mk_concat(n67, n68);
    let n70 = b.mk_range_u8(0xA1, 0xA1);
    let n71 = b.mk_ranges_u8(&[(0x80, 0x9B), (0xA0, 0xAA), (0xB0, 0xBF)]);
    let n72 = b.mk_concat(n70, n71);
    let n73 = b.mk_range_u8(0xA2, 0xA2);
    let n74 = b.mk_ranges_u8(&[(0x80, 0x87), (0x89, 0x8E), (0x97, 0xBF)]);
    let n75 = b.mk_concat(n73, n74);
    let n76 = b.mk_range_u8(0xA3, 0xA3);
    let n77 = b.mk_ranges_u8(&[(0x80, 0xA1), (0xA3, 0xBF)]);
    let n78 = b.mk_concat(n76, n77);
    let n79 = b.mk_range_u8(0xA4, 0xA4);
    let n80 = b.mk_concat(n79, n8);
    let n81 = b.mk_range_u8(0xA5, 0xA5);
    let n82 = b.mk_ranges_u8(&[(0x80, 0xA3), (0xA6, 0xAF), (0xB1, 0xBF)]);
    let n83 = b.mk_concat(n81, n82);
    let n84 = b.mk_range_u8(0xA6, 0xA6);
    let n85 = b.mk_ranges_u8(&[
        (0x80, 0x83),
        (0x85, 0x8C),
        (0x8F, 0x90),
        (0x93, 0xA8),
        (0xAA, 0xB0),
        (0xB2, 0xB2),
        (0xB6, 0xB9),
        (0xBC, 0xBF),
    ]);
    let n86 = b.mk_concat(n84, n85);
    let n87 = b.mk_range_u8(0xA7, 0xA7);
    let n88 = b.mk_ranges_u8(&[
        (0x80, 0x84),
        (0x87, 0x88),
        (0x8B, 0x8E),
        (0x97, 0x97),
        (0x9C, 0x9D),
        (0x9F, 0xA3),
        (0xA6, 0xB1),
        (0xBC, 0xBC),
        (0xBE, 0xBE),
    ]);
    let n89 = b.mk_concat(n87, n88);
    let n90 = b.mk_range_u8(0xA8, 0xA8);
    let n91 = b.mk_ranges_u8(&[
        (0x81, 0x83),
        (0x85, 0x8A),
        (0x8F, 0x90),
        (0x93, 0xA8),
        (0xAA, 0xB0),
        (0xB2, 0xB3),
        (0xB5, 0xB6),
        (0xB8, 0xB9),
        (0xBC, 0xBC),
        (0xBE, 0xBF),
    ]);
    let n92 = b.mk_concat(n90, n91);
    let n93 = b.mk_range_u8(0xA9, 0xA9);
    let n94 = b.mk_ranges_u8(&[
        (0x80, 0x82),
        (0x87, 0x88),
        (0x8B, 0x8D),
        (0x91, 0x91),
        (0x99, 0x9C),
        (0x9E, 0x9E),
        (0xA6, 0xB5),
    ]);
    let n95 = b.mk_concat(n93, n94);
    let n96 = b.mk_range_u8(0xAA, 0xAA);
    let n97 = b.mk_ranges_u8(&[
        (0x81, 0x83),
        (0x85, 0x8D),
        (0x8F, 0x91),
        (0x93, 0xA8),
        (0xAA, 0xB0),
        (0xB2, 0xB3),
        (0xB5, 0xB9),
        (0xBC, 0xBF),
    ]);
    let n98 = b.mk_concat(n96, n97);
    let n99 = b.mk_range_u8(0xAB, 0xAB);
    let n100 = b.mk_ranges_u8(&[
        (0x80, 0x85),
        (0x87, 0x89),
        (0x8B, 0x8D),
        (0x90, 0x90),
        (0xA0, 0xA3),
        (0xA6, 0xAF),
        (0xB9, 0xBF),
    ]);
    let n101 = b.mk_concat(n99, n100);
    let n102 = b.mk_range_u8(0xAC, 0xAC);
    let n103 = b.mk_ranges_u8(&[
        (0x81, 0x83),
        (0x85, 0x8C),
        (0x8F, 0x90),
        (0x93, 0xA8),
        (0xAA, 0xB0),
        (0xB2, 0xB3),
        (0xB5, 0xB9),
        (0xBC, 0xBF),
    ]);
    let n104 = b.mk_concat(n102, n103);
    let n105 = b.mk_range_u8(0xAD, 0xAD);
    let n106 = b.mk_ranges_u8(&[
        (0x80, 0x84),
        (0x87, 0x88),
        (0x8B, 0x8D),
        (0x95, 0x97),
        (0x9C, 0x9D),
        (0x9F, 0xA3),
        (0xA6, 0xAF),
        (0xB1, 0xB1),
    ]);
    let n107 = b.mk_concat(n105, n106);
    let n108 = b.mk_range_u8(0xAE, 0xAE);
    let n109 = b.mk_ranges_u8(&[
        (0x82, 0x83),
        (0x85, 0x8A),
        (0x8E, 0x90),
        (0x92, 0x95),
        (0x99, 0x9A),
        (0x9C, 0x9C),
        (0x9E, 0x9F),
        (0xA3, 0xA4),
        (0xA8, 0xAA),
        (0xAE, 0xB9),
        (0xBE, 0xBF),
    ]);
    let n110 = b.mk_concat(n108, n109);
    let n111 = b.mk_range_u8(0xAF, 0xAF);
    let n112 = b.mk_ranges_u8(&[
        (0x80, 0x82),
        (0x86, 0x88),
        (0x8A, 0x8D),
        (0x90, 0x90),
        (0x97, 0x97),
        (0xA6, 0xAF),
    ]);
    let n113 = b.mk_concat(n111, n112);
    let n114 = b.mk_range_u8(0xB0, 0xB0);
    let n115 = b.mk_ranges_u8(&[
        (0x80, 0x8C),
        (0x8E, 0x90),
        (0x92, 0xA8),
        (0xAA, 0xB9),
        (0xBC, 0xBF),
    ]);
    let n116 = b.mk_concat(n114, n115);
    let n117 = b.mk_range_u8(0xB1, 0xB1);
    let n118 = b.mk_ranges_u8(&[
        (0x80, 0x84),
        (0x86, 0x88),
        (0x8A, 0x8D),
        (0x95, 0x96),
        (0x98, 0x9A),
        (0x9D, 0x9D),
        (0xA0, 0xA3),
        (0xA6, 0xAF),
    ]);
    let n119 = b.mk_concat(n117, n118);
    let n120 = b.mk_range_u8(0xB2, 0xB2);
    let n121 = b.mk_ranges_u8(&[
        (0x80, 0x83),
        (0x85, 0x8C),
        (0x8E, 0x90),
        (0x92, 0xA8),
        (0xAA, 0xB3),
        (0xB5, 0xB9),
        (0xBC, 0xBF),
    ]);
    let n122 = b.mk_concat(n120, n121);
    let n123 = b.mk_range_u8(0xB3, 0xB3);
    let n124 = b.mk_ranges_u8(&[
        (0x80, 0x84),
        (0x86, 0x88),
        (0x8A, 0x8D),
        (0x95, 0x96),
        (0x9D, 0x9E),
        (0xA0, 0xA3),
        (0xA6, 0xAF),
        (0xB1, 0xB3),
    ]);
    let n125 = b.mk_concat(n123, n124);
    let n126 = b.mk_range_u8(0xB4, 0xB4);
    let n127 = b.mk_ranges_u8(&[(0x80, 0x8C), (0x8E, 0x90), (0x92, 0xBF)]);
    let n128 = b.mk_concat(n126, n127);
    let n129 = b.mk_range_u8(0xB5, 0xB5);
    let n130 = b.mk_ranges_u8(&[
        (0x80, 0x84),
        (0x86, 0x88),
        (0x8A, 0x8E),
        (0x94, 0x97),
        (0x9F, 0xA3),
        (0xA6, 0xAF),
        (0xBA, 0xBF),
    ]);
    let n131 = b.mk_concat(n129, n130);
    let n132 = b.mk_range_u8(0xB6, 0xB6);
    let n133 = b.mk_ranges_u8(&[
        (0x81, 0x83),
        (0x85, 0x96),
        (0x9A, 0xB1),
        (0xB3, 0xBB),
        (0xBD, 0xBD),
    ]);
    let n134 = b.mk_concat(n132, n133);
    let n135 = b.mk_range_u8(0xB7, 0xB7);
    let n136 = b.mk_ranges_u8(&[
        (0x80, 0x86),
        (0x8A, 0x8A),
        (0x8F, 0x94),
        (0x96, 0x96),
        (0x98, 0x9F),
        (0xA6, 0xAF),
        (0xB2, 0xB3),
    ]);
    let n137 = b.mk_concat(n135, n136);
    let n138 = b.mk_range_u8(0xB8, 0xB8);
    let n139 = b.mk_range_u8(0x81, 0xBA);
    let n140 = b.mk_concat(n138, n139);
    let n141 = b.mk_range_u8(0xB9, 0xB9);
    let n142 = b.mk_ranges_u8(&[(0x80, 0x8E), (0x90, 0x99)]);
    let n143 = b.mk_concat(n141, n142);
    let n144 = b.mk_range_u8(0xBA, 0xBA);
    let n145 = b.mk_ranges_u8(&[
        (0x81, 0x82),
        (0x84, 0x84),
        (0x86, 0x8A),
        (0x8C, 0xA3),
        (0xA5, 0xA5),
        (0xA7, 0xBD),
    ]);
    let n146 = b.mk_concat(n144, n145);
    let n147 = b.mk_range_u8(0xBB, 0xBB);
    let n148 = b.mk_ranges_u8(&[
        (0x80, 0x84),
        (0x86, 0x86),
        (0x88, 0x8E),
        (0x90, 0x99),
        (0x9C, 0x9F),
    ]);
    let n149 = b.mk_concat(n147, n148);
    let n150 = b.mk_range_u8(0xBC, 0xBC);
    let n151 = b.mk_ranges_u8(&[
        (0x80, 0x80),
        (0x98, 0x99),
        (0xA0, 0xA9),
        (0xB5, 0xB5),
        (0xB7, 0xB7),
        (0xB9, 0xB9),
        (0xBE, 0xBF),
    ]);
    let n152 = b.mk_concat(n150, n151);
    let n153 = b.mk_range_u8(0xBD, 0xBD);
    let n154 = b.mk_ranges_u8(&[(0x80, 0x87), (0x89, 0xAC), (0xB1, 0xBF)]);
    let n155 = b.mk_concat(n153, n154);
    let n156 = b.mk_range_u8(0xBE, 0xBE);
    let n157 = b.mk_ranges_u8(&[(0x80, 0x84), (0x86, 0x97), (0x99, 0xBC)]);
    let n158 = b.mk_concat(n156, n157);
    let n159 = b.mk_range_u8(0xBF, 0xBF);
    let n160 = b.mk_range_u8(0x86, 0x86);
    let n161 = b.mk_concat(n159, n160);
    let n162 = b.mk_union(n158, n161);
    let n163 = b.mk_union(n155, n162);
    let n164 = b.mk_union(n152, n163);
    let n165 = b.mk_union(n149, n164);
    let n166 = b.mk_union(n146, n165);
    let n167 = b.mk_union(n143, n166);
    let n168 = b.mk_union(n140, n167);
    let n169 = b.mk_union(n137, n168);
    let n170 = b.mk_union(n134, n169);
    let n171 = b.mk_union(n131, n170);
    let n172 = b.mk_union(n128, n171);
    let n173 = b.mk_union(n125, n172);
    let n174 = b.mk_union(n122, n173);
    let n175 = b.mk_union(n119, n174);
    let n176 = b.mk_union(n116, n175);
    let n177 = b.mk_union(n113, n176);
    let n178 = b.mk_union(n110, n177);
    let n179 = b.mk_union(n107, n178);
    let n180 = b.mk_union(n104, n179);
    let n181 = b.mk_union(n101, n180);
    let n182 = b.mk_union(n98, n181);
    let n183 = b.mk_union(n95, n182);
    let n184 = b.mk_union(n92, n183);
    let n185 = b.mk_union(n89, n184);
    let n186 = b.mk_union(n86, n185);
    let n187 = b.mk_union(n83, n186);
    let n188 = b.mk_union(n80, n187);
    let n189 = b.mk_union(n78, n188);
    let n190 = b.mk_union(n75, n189);
    let n191 = b.mk_union(n72, n190);
    let n192 = b.mk_union(n69, n191);
    let n193 = b.mk_concat(n66, n192);
    let n194 = b.mk_range_u8(0xE1, 0xE1);
    let n195 = b.mk_range_u8(0x80, 0x80);
    let n196 = b.mk_concat(n195, n8);
    let n197 = b.mk_range_u8(0x81, 0x81);
    let n198 = b.mk_ranges_u8(&[(0x80, 0x89), (0x90, 0xBF)]);
    let n199 = b.mk_concat(n197, n198);
    let n200 = b.mk_range_u8(0x82, 0x82);
    let n201 = b.mk_ranges_u8(&[(0x80, 0x9D), (0xA0, 0xBF)]);
    let n202 = b.mk_concat(n200, n201);
    let n203 = b.mk_range_u8(0x83, 0x83);
    let n204 = b.mk_ranges_u8(&[
        (0x80, 0x85),
        (0x87, 0x87),
        (0x8D, 0x8D),
        (0x90, 0xBA),
        (0xBC, 0xBF),
    ]);
    let n205 = b.mk_concat(n203, n204);
    let n206 = b.mk_range_u8(0x84, 0x88);
    let n207 = b.mk_concat(n206, n8);
    let n208 = b.mk_range_u8(0x89, 0x89);
    let n209 = b.mk_ranges_u8(&[
        (0x80, 0x88),
        (0x8A, 0x8D),
        (0x90, 0x96),
        (0x98, 0x98),
        (0x9A, 0x9D),
        (0xA0, 0xBF),
    ]);
    let n210 = b.mk_concat(n208, n209);
    let n211 = b.mk_range_u8(0x8A, 0x8A);
    let n212 = b.mk_ranges_u8(&[
        (0x80, 0x88),
        (0x8A, 0x8D),
        (0x90, 0xB0),
        (0xB2, 0xB5),
        (0xB8, 0xBE),
    ]);
    let n213 = b.mk_concat(n211, n212);
    let n214 = b.mk_range_u8(0x8B, 0x8B);
    let n215 = b.mk_ranges_u8(&[(0x80, 0x80), (0x82, 0x85), (0x88, 0x96), (0x98, 0xBF)]);
    let n216 = b.mk_concat(n214, n215);
    let n217 = b.mk_range_u8(0x8C, 0x8C);
    let n218 = b.mk_ranges_u8(&[(0x80, 0x90), (0x92, 0x95), (0x98, 0xBF)]);
    let n219 = b.mk_concat(n217, n218);
    let n220 = b.mk_range_u8(0x8D, 0x8D);
    let n221 = b.mk_ranges_u8(&[(0x80, 0x9A), (0x9D, 0x9F)]);
    let n222 = b.mk_concat(n220, n221);
    let n223 = b.mk_range_u8(0x8E, 0x8E);
    let n224 = b.mk_ranges_u8(&[(0x80, 0x8F), (0xA0, 0xBF)]);
    let n225 = b.mk_concat(n223, n224);
    let n226 = b.mk_range_u8(0x8F, 0x8F);
    let n227 = b.mk_ranges_u8(&[(0x80, 0xB5), (0xB8, 0xBD)]);
    let n228 = b.mk_concat(n226, n227);
    let n229 = b.mk_range_u8(0x90, 0x90);
    let n230 = b.mk_range_u8(0x81, 0xBF);
    let n231 = b.mk_concat(n229, n230);
    let n232 = b.mk_range_u8(0x91, 0x98);
    let n233 = b.mk_concat(n232, n8);
    let n234 = b.mk_range_u8(0x99, 0x99);
    let n235 = b.mk_ranges_u8(&[(0x80, 0xAC), (0xAF, 0xBF)]);
    let n236 = b.mk_concat(n234, n235);
    let n237 = b.mk_range_u8(0x9A, 0x9A);
    let n238 = b.mk_ranges_u8(&[(0x81, 0x9A), (0xA0, 0xBF)]);
    let n239 = b.mk_concat(n237, n238);
    let n240 = b.mk_range_u8(0x9B, 0x9B);
    let n241 = b.mk_ranges_u8(&[(0x80, 0xAA), (0xAE, 0xB8)]);
    let n242 = b.mk_concat(n240, n241);
    let n243 = b.mk_range_u8(0x9C, 0x9C);
    let n244 = b.mk_ranges_u8(&[(0x80, 0x95), (0x9F, 0xB4)]);
    let n245 = b.mk_concat(n243, n244);
    let n246 = b.mk_range_u8(0x9D, 0x9D);
    let n247 = b.mk_ranges_u8(&[(0x80, 0x93), (0xA0, 0xAC), (0xAE, 0xB0), (0xB2, 0xB3)]);
    let n248 = b.mk_concat(n246, n247);
    let n249 = b.mk_range_u8(0x9E, 0x9E);
    let n250 = b.mk_concat(n249, n8);
    let n251 = b.mk_range_u8(0x9F, 0x9F);
    let n252 = b.mk_ranges_u8(&[(0x80, 0x93), (0x97, 0x97), (0x9C, 0x9D), (0xA0, 0xA9)]);
    let n253 = b.mk_concat(n251, n252);
    let n254 = b.mk_ranges_u8(&[(0x8B, 0x8D), (0x8F, 0x99), (0xA0, 0xBF)]);
    let n255 = b.mk_concat(n67, n254);
    let n256 = b.mk_range_u8(0x80, 0xB8);
    let n257 = b.mk_concat(n70, n256);
    let n258 = b.mk_ranges_u8(&[(0x80, 0xAA), (0xB0, 0xBF)]);
    let n259 = b.mk_concat(n73, n258);
    let n260 = b.mk_range_u8(0x80, 0xB5);
    let n261 = b.mk_concat(n76, n260);
    let n262 = b.mk_ranges_u8(&[(0x80, 0x9E), (0xA0, 0xAB), (0xB0, 0xBB)]);
    let n263 = b.mk_concat(n79, n262);
    let n264 = b.mk_ranges_u8(&[(0x86, 0xAD), (0xB0, 0xB4)]);
    let n265 = b.mk_concat(n81, n264);
    let n266 = b.mk_ranges_u8(&[(0x80, 0xAB), (0xB0, 0xBF)]);
    let n267 = b.mk_concat(n84, n266);
    let n268 = b.mk_ranges_u8(&[(0x80, 0x89), (0x90, 0x99)]);
    let n269 = b.mk_concat(n87, n268);
    let n270 = b.mk_ranges_u8(&[(0x80, 0x9B), (0xA0, 0xBF)]);
    let n271 = b.mk_concat(n90, n270);
    let n272 = b.mk_ranges_u8(&[(0x80, 0x9E), (0xA0, 0xBC), (0xBF, 0xBF)]);
    let n273 = b.mk_concat(n93, n272);
    let n274 = b.mk_ranges_u8(&[(0x80, 0x89), (0x90, 0x99), (0xA7, 0xA7), (0xB0, 0xBF)]);
    let n275 = b.mk_concat(n96, n274);
    let n276 = b.mk_range_u8(0x80, 0x8E);
    let n277 = b.mk_concat(n99, n276);
    let n278 = b.mk_concat(n102, n8);
    let n279 = b.mk_ranges_u8(&[(0x80, 0x8C), (0x90, 0x99), (0xAB, 0xB3)]);
    let n280 = b.mk_concat(n105, n279);
    let n281 = b.mk_concat(n108, n8);
    let n282 = b.mk_range_u8(0x80, 0xB3);
    let n283 = b.mk_concat(n111, n282);
    let n284 = b.mk_range_u8(0x80, 0xB7);
    let n285 = b.mk_concat(n114, n284);
    let n286 = b.mk_ranges_u8(&[(0x80, 0x89), (0x8D, 0xBD)]);
    let n287 = b.mk_concat(n117, n286);
    let n288 = b.mk_ranges_u8(&[(0x80, 0x8A), (0x90, 0xBA), (0xBD, 0xBF)]);
    let n289 = b.mk_concat(n120, n288);
    let n290 = b.mk_ranges_u8(&[(0x90, 0x92), (0x94, 0xBA)]);
    let n291 = b.mk_concat(n123, n290);
    let n292 = b.mk_range_u8(0xB4, 0xBB);
    let n293 = b.mk_concat(n292, n8);
    let n294 = b.mk_ranges_u8(&[(0x80, 0x95), (0x98, 0x9D), (0xA0, 0xBF)]);
    let n295 = b.mk_concat(n150, n294);
    let n296 = b.mk_ranges_u8(&[
        (0x80, 0x85),
        (0x88, 0x8D),
        (0x90, 0x97),
        (0x99, 0x99),
        (0x9B, 0x9B),
        (0x9D, 0x9D),
        (0x9F, 0xBD),
    ]);
    let n297 = b.mk_concat(n153, n296);
    let n298 = b.mk_ranges_u8(&[(0x80, 0xB4), (0xB6, 0xBC), (0xBE, 0xBE)]);
    let n299 = b.mk_concat(n156, n298);
    let n300 = b.mk_ranges_u8(&[
        (0x82, 0x84),
        (0x86, 0x8C),
        (0x90, 0x93),
        (0x96, 0x9B),
        (0xA0, 0xAC),
        (0xB2, 0xB4),
        (0xB6, 0xBC),
    ]);
    let n301 = b.mk_concat(n159, n300);
    let n302 = b.mk_union(n299, n301);
    let n303 = b.mk_union(n297, n302);
    let n304 = b.mk_union(n295, n303);
    let n305 = b.mk_union(n293, n304);
    let n306 = b.mk_union(n291, n305);
    let n307 = b.mk_union(n289, n306);
    let n308 = b.mk_union(n287, n307);
    let n309 = b.mk_union(n285, n308);
    let n310 = b.mk_union(n283, n309);
    let n311 = b.mk_union(n281, n310);
    let n312 = b.mk_union(n280, n311);
    let n313 = b.mk_union(n278, n312);
    let n314 = b.mk_union(n277, n313);
    let n315 = b.mk_union(n275, n314);
    let n316 = b.mk_union(n273, n315);
    let n317 = b.mk_union(n271, n316);
    let n318 = b.mk_union(n269, n317);
    let n319 = b.mk_union(n267, n318);
    let n320 = b.mk_union(n265, n319);
    let n321 = b.mk_union(n263, n320);
    let n322 = b.mk_union(n261, n321);
    let n323 = b.mk_union(n259, n322);
    let n324 = b.mk_union(n257, n323);
    let n325 = b.mk_union(n255, n324);
    let n326 = b.mk_union(n253, n325);
    let n327 = b.mk_union(n250, n326);
    let n328 = b.mk_union(n248, n327);
    let n329 = b.mk_union(n245, n328);
    let n330 = b.mk_union(n242, n329);
    let n331 = b.mk_union(n239, n330);
    let n332 = b.mk_union(n236, n331);
    let n333 = b.mk_union(n233, n332);
    let n334 = b.mk_union(n231, n333);
    let n335 = b.mk_union(n228, n334);
    let n336 = b.mk_union(n225, n335);
    let n337 = b.mk_union(n222, n336);
    let n338 = b.mk_union(n219, n337);
    let n339 = b.mk_union(n216, n338);
    let n340 = b.mk_union(n213, n339);
    let n341 = b.mk_union(n210, n340);
    let n342 = b.mk_union(n207, n341);
    let n343 = b.mk_union(n205, n342);
    let n344 = b.mk_union(n202, n343);
    let n345 = b.mk_union(n199, n344);
    let n346 = b.mk_union(n196, n345);
    let n347 = b.mk_concat(n194, n346);
    let n348 = b.mk_range_u8(0xE2, 0xE2);
    let n349 = b.mk_ranges_u8(&[(0x8C, 0x8D), (0xBF, 0xBF)]);
    let n350 = b.mk_concat(n195, n349);
    let n351 = b.mk_ranges_u8(&[(0x80, 0x80), (0x94, 0x94), (0xB1, 0xB1), (0xBF, 0xBF)]);
    let n352 = b.mk_concat(n197, n351);
    let n353 = b.mk_range_u8(0x90, 0x9C);
    let n354 = b.mk_concat(n200, n353);
    let n355 = b.mk_range_u8(0x90, 0xB0);
    let n356 = b.mk_concat(n203, n355);
    let n357 = b.mk_range_u8(0x84, 0x84);
    let n358 = b.mk_ranges_u8(&[
        (0x82, 0x82),
        (0x87, 0x87),
        (0x8A, 0x93),
        (0x95, 0x95),
        (0x99, 0x9D),
        (0xA4, 0xA4),
        (0xA6, 0xA6),
        (0xA8, 0xA8),
        (0xAA, 0xAD),
        (0xAF, 0xB9),
        (0xBC, 0xBF),
    ]);
    let n359 = b.mk_concat(n357, n358);
    let n360 = b.mk_range_u8(0x85, 0x85);
    let n361 = b.mk_ranges_u8(&[(0x85, 0x89), (0x8E, 0x8E), (0xA0, 0xBF)]);
    let n362 = b.mk_concat(n360, n361);
    let n363 = b.mk_range_u8(0x80, 0x88);
    let n364 = b.mk_concat(n160, n363);
    let n365 = b.mk_range_u8(0x92, 0x92);
    let n366 = b.mk_range_u8(0xB6, 0xBF);
    let n367 = b.mk_concat(n365, n366);
    let n368 = b.mk_range_u8(0x93, 0x93);
    let n369 = b.mk_range_u8(0x80, 0xA9);
    let n370 = b.mk_concat(n368, n369);
    let n371 = b.mk_range_u8(0xB0, 0xB2);
    let n372 = b.mk_concat(n371, n8);
    let n373 = b.mk_ranges_u8(&[(0x80, 0xA4), (0xAB, 0xB3)]);
    let n374 = b.mk_concat(n123, n373);
    let n375 = b.mk_ranges_u8(&[(0x80, 0xA5), (0xA7, 0xA7), (0xAD, 0xAD), (0xB0, 0xBF)]);
    let n376 = b.mk_concat(n126, n375);
    let n377 = b.mk_ranges_u8(&[(0x80, 0xA7), (0xAF, 0xAF), (0xBF, 0xBF)]);
    let n378 = b.mk_concat(n129, n377);
    let n379 = b.mk_ranges_u8(&[
        (0x80, 0x96),
        (0xA0, 0xA6),
        (0xA8, 0xAE),
        (0xB0, 0xB6),
        (0xB8, 0xBE),
    ]);
    let n380 = b.mk_concat(n132, n379);
    let n381 = b.mk_ranges_u8(&[
        (0x80, 0x86),
        (0x88, 0x8E),
        (0x90, 0x96),
        (0x98, 0x9E),
        (0xA0, 0xBF),
    ]);
    let n382 = b.mk_concat(n135, n381);
    let n383 = b.mk_concat(n138, n111);
    let n384 = b.mk_union(n382, n383);
    let n385 = b.mk_union(n380, n384);
    let n386 = b.mk_union(n378, n385);
    let n387 = b.mk_union(n376, n386);
    let n388 = b.mk_union(n374, n387);
    let n389 = b.mk_union(n372, n388);
    let n390 = b.mk_union(n370, n389);
    let n391 = b.mk_union(n367, n390);
    let n392 = b.mk_union(n364, n391);
    let n393 = b.mk_union(n362, n392);
    let n394 = b.mk_union(n359, n393);
    let n395 = b.mk_union(n356, n394);
    let n396 = b.mk_union(n354, n395);
    let n397 = b.mk_union(n352, n396);
    let n398 = b.mk_union(n350, n397);
    let n399 = b.mk_concat(n348, n398);
    let n400 = b.mk_range_u8(0xE3, 0xE3);
    let n401 = b.mk_ranges_u8(&[(0x85, 0x87), (0xA1, 0xAF), (0xB1, 0xB5), (0xB8, 0xBC)]);
    let n402 = b.mk_concat(n195, n401);
    let n403 = b.mk_concat(n197, n230);
    let n404 = b.mk_ranges_u8(&[(0x80, 0x96), (0x99, 0x9A), (0x9D, 0x9F), (0xA1, 0xBF)]);
    let n405 = b.mk_concat(n200, n404);
    let n406 = b.mk_ranges_u8(&[(0x80, 0xBA), (0xBC, 0xBF)]);
    let n407 = b.mk_concat(n203, n406);
    let n408 = b.mk_ranges_u8(&[(0x85, 0xAF), (0xB1, 0xBF)]);
    let n409 = b.mk_concat(n357, n408);
    let n410 = b.mk_concat(n360, n8);
    let n411 = b.mk_ranges_u8(&[(0x80, 0x8E), (0xA0, 0xBF)]);
    let n412 = b.mk_concat(n160, n411);
    let n413 = b.mk_range_u8(0x87, 0x87);
    let n414 = b.mk_range_u8(0xB0, 0xBF);
    let n415 = b.mk_concat(n413, n414);
    let n416 = b.mk_concat(n55, n8);
    let n417 = b.mk_union(n415, n416);
    let n418 = b.mk_union(n412, n417);
    let n419 = b.mk_union(n410, n418);
    let n420 = b.mk_union(n409, n419);
    let n421 = b.mk_union(n407, n420);
    let n422 = b.mk_union(n405, n421);
    let n423 = b.mk_union(n403, n422);
    let n424 = b.mk_union(n402, n423);
    let n425 = b.mk_concat(n400, n424);
    let n426 = b.mk_range_u8(0xE4, 0xE4);
    let n427 = b.mk_range_u8(0x80, 0xB6);
    let n428 = b.mk_concat(n427, n8);
    let n429 = b.mk_range_u8(0xB8, 0xBF);
    let n430 = b.mk_concat(n429, n8);
    let n431 = b.mk_union(n428, n430);
    let n432 = b.mk_concat(n426, n431);
    let n433 = b.mk_range_u8(0xE5, 0xE9);
    let n434 = b.mk_concat(n8, n8);
    let n435 = b.mk_concat(n433, n434);
    let n436 = b.mk_range_u8(0xEA, 0xEA);
    let n437 = b.mk_range_u8(0x80, 0x91);
    let n438 = b.mk_concat(n437, n8);
    let n439 = b.mk_range_u8(0x80, 0x8C);
    let n440 = b.mk_concat(n365, n439);
    let n441 = b.mk_range_u8(0x90, 0xBD);
    let n442 = b.mk_concat(n368, n441);
    let n443 = b.mk_range_u8(0x94, 0x97);
    let n444 = b.mk_concat(n443, n8);
    let n445 = b.mk_range_u8(0x98, 0x98);
    let n446 = b.mk_ranges_u8(&[(0x80, 0x8C), (0x90, 0xAB)]);
    let n447 = b.mk_concat(n445, n446);
    let n448 = b.mk_ranges_u8(&[(0x80, 0xB2), (0xB4, 0xBD), (0xBF, 0xBF)]);
    let n449 = b.mk_concat(n234, n448);
    let n450 = b.mk_concat(n237, n8);
    let n451 = b.mk_concat(n240, n61);
    let n452 = b.mk_ranges_u8(&[(0x97, 0x9F), (0xA2, 0xBF)]);
    let n453 = b.mk_concat(n243, n452);
    let n454 = b.mk_concat(n246, n8);
    let n455 = b.mk_ranges_u8(&[(0x80, 0x88), (0x8B, 0xBF)]);
    let n456 = b.mk_concat(n249, n455);
    let n457 = b.mk_ranges_u8(&[
        (0x80, 0x8D),
        (0x90, 0x91),
        (0x93, 0x93),
        (0x95, 0x9C),
        (0xB2, 0xBF),
    ]);
    let n458 = b.mk_concat(n251, n457);
    let n459 = b.mk_ranges_u8(&[(0x80, 0xA7), (0xAC, 0xAC)]);
    let n460 = b.mk_concat(n67, n459);
    let n461 = b.mk_concat(n70, n282);
    let n462 = b.mk_concat(n73, n8);
    let n463 = b.mk_ranges_u8(&[
        (0x80, 0x85),
        (0x90, 0x99),
        (0xA0, 0xB7),
        (0xBB, 0xBB),
        (0xBD, 0xBF),
    ]);
    let n464 = b.mk_concat(n76, n463);
    let n465 = b.mk_ranges_u8(&[(0x80, 0xAD), (0xB0, 0xBF)]);
    let n466 = b.mk_concat(n79, n465);
    let n467 = b.mk_ranges_u8(&[(0x80, 0x93), (0xA0, 0xBC)]);
    let n468 = b.mk_concat(n81, n467);
    let n469 = b.mk_concat(n84, n8);
    let n470 = b.mk_ranges_u8(&[(0x80, 0x80), (0x8F, 0x99), (0xA0, 0xBE)]);
    let n471 = b.mk_concat(n87, n470);
    let n472 = b.mk_concat(n90, n427);
    let n473 = b.mk_ranges_u8(&[(0x80, 0x8D), (0x90, 0x99), (0xA0, 0xB6), (0xBA, 0xBF)]);
    let n474 = b.mk_concat(n93, n473);
    let n475 = b.mk_concat(n96, n8);
    let n476 = b.mk_ranges_u8(&[(0x80, 0x82), (0x9B, 0x9D), (0xA0, 0xAF), (0xB2, 0xB6)]);
    let n477 = b.mk_concat(n99, n476);
    let n478 = b.mk_ranges_u8(&[
        (0x81, 0x86),
        (0x89, 0x8E),
        (0x91, 0x96),
        (0xA0, 0xA6),
        (0xA8, 0xAE),
        (0xB0, 0xBF),
    ]);
    let n479 = b.mk_concat(n102, n478);
    let n480 = b.mk_ranges_u8(&[(0x80, 0x9A), (0x9C, 0xA9), (0xB0, 0xBF)]);
    let n481 = b.mk_concat(n105, n480);
    let n482 = b.mk_ranges_u8(&[(0x80, 0xAA), (0xAC, 0xAD), (0xB0, 0xB9)]);
    let n483 = b.mk_concat(n111, n482);
    let n484 = b.mk_concat(n414, n8);
    let n485 = b.mk_union(n483, n484);
    let n486 = b.mk_union(n481, n485);
    let n487 = b.mk_union(n479, n486);
    let n488 = b.mk_union(n477, n487);
    let n489 = b.mk_union(n475, n488);
    let n490 = b.mk_union(n474, n489);
    let n491 = b.mk_union(n472, n490);
    let n492 = b.mk_union(n471, n491);
    let n493 = b.mk_union(n469, n492);
    let n494 = b.mk_union(n468, n493);
    let n495 = b.mk_union(n466, n494);
    let n496 = b.mk_union(n464, n495);
    let n497 = b.mk_union(n462, n496);
    let n498 = b.mk_union(n461, n497);
    let n499 = b.mk_union(n460, n498);
    let n500 = b.mk_union(n458, n499);
    let n501 = b.mk_union(n456, n500);
    let n502 = b.mk_union(n454, n501);
    let n503 = b.mk_union(n453, n502);
    let n504 = b.mk_union(n451, n503);
    let n505 = b.mk_union(n450, n504);
    let n506 = b.mk_union(n449, n505);
    let n507 = b.mk_union(n447, n506);
    let n508 = b.mk_union(n444, n507);
    let n509 = b.mk_union(n442, n508);
    let n510 = b.mk_union(n440, n509);
    let n511 = b.mk_union(n438, n510);
    let n512 = b.mk_union(n281, n511);
    let n513 = b.mk_concat(n436, n512);
    let n514 = b.mk_range_u8(0xEB, 0xEC);
    let n515 = b.mk_concat(n514, n434);
    let n516 = b.mk_range_u8(0xED, 0xED);
    let n517 = b.mk_range_u8(0x80, 0x9D);
    let n518 = b.mk_concat(n517, n8);
    let n519 = b.mk_ranges_u8(&[(0x80, 0xA3), (0xB0, 0xBF)]);
    let n520 = b.mk_concat(n249, n519);
    let n521 = b.mk_ranges_u8(&[(0x80, 0x86), (0x8B, 0xBB)]);
    let n522 = b.mk_concat(n251, n521);
    let n523 = b.mk_union(n520, n522);
    let n524 = b.mk_union(n518, n523);
    let n525 = b.mk_concat(n516, n524);
    let n526 = b.mk_range_u8(0xEF, 0xEF);
    let n527 = b.mk_range_u8(0xA4, 0xA8);
    let n528 = b.mk_concat(n527, n8);
    let n529 = b.mk_concat(n93, n465);
    let n530 = b.mk_range_u8(0x80, 0x99);
    let n531 = b.mk_concat(n99, n530);
    let n532 = b.mk_ranges_u8(&[
        (0x80, 0x86),
        (0x93, 0x97),
        (0x9D, 0xA8),
        (0xAA, 0xB6),
        (0xB8, 0xBC),
        (0xBE, 0xBE),
    ]);
    let n533 = b.mk_concat(n102, n532);
    let n534 = b.mk_ranges_u8(&[(0x80, 0x81), (0x83, 0x84), (0x86, 0xBF)]);
    let n535 = b.mk_concat(n105, n534);
    let n536 = b.mk_concat(n108, n61);
    let n537 = b.mk_range_u8(0x93, 0xBF);
    let n538 = b.mk_concat(n111, n537);
    let n539 = b.mk_range_u8(0xB0, 0xB3);
    let n540 = b.mk_concat(n539, n8);
    let n541 = b.mk_range_u8(0x80, 0xBD);
    let n542 = b.mk_concat(n126, n541);
    let n543 = b.mk_concat(n129, n55);
    let n544 = b.mk_ranges_u8(&[(0x80, 0x8F), (0x92, 0xBF)]);
    let n545 = b.mk_concat(n132, n544);
    let n546 = b.mk_ranges_u8(&[(0x80, 0x87), (0xB0, 0xBB)]);
    let n547 = b.mk_concat(n135, n546);
    let n548 = b.mk_ranges_u8(&[(0x80, 0x8F), (0xA0, 0xAF), (0xB3, 0xB4)]);
    let n549 = b.mk_concat(n138, n548);
    let n550 = b.mk_ranges_u8(&[(0x8D, 0x8F), (0xB0, 0xB4), (0xB6, 0xBF)]);
    let n551 = b.mk_concat(n141, n550);
    let n552 = b.mk_concat(n144, n8);
    let n553 = b.mk_range_u8(0x80, 0xBC);
    let n554 = b.mk_concat(n147, n553);
    let n555 = b.mk_ranges_u8(&[(0x90, 0x99), (0xA1, 0xBA), (0xBF, 0xBF)]);
    let n556 = b.mk_concat(n150, n555);
    let n557 = b.mk_ranges_u8(&[(0x81, 0x9A), (0xA6, 0xBF)]);
    let n558 = b.mk_concat(n153, n557);
    let n559 = b.mk_range_u8(0x80, 0xBE);
    let n560 = b.mk_concat(n156, n559);
    let n561 = b.mk_ranges_u8(&[(0x82, 0x87), (0x8A, 0x8F), (0x92, 0x97), (0x9A, 0x9C)]);
    let n562 = b.mk_concat(n159, n561);
    let n563 = b.mk_union(n560, n562);
    let n564 = b.mk_union(n558, n563);
    let n565 = b.mk_union(n556, n564);
    let n566 = b.mk_union(n554, n565);
    let n567 = b.mk_union(n552, n566);
    let n568 = b.mk_union(n551, n567);
    let n569 = b.mk_union(n549, n568);
    let n570 = b.mk_union(n547, n569);
    let n571 = b.mk_union(n545, n570);
    let n572 = b.mk_union(n543, n571);
    let n573 = b.mk_union(n542, n572);
    let n574 = b.mk_union(n540, n573);
    let n575 = b.mk_union(n538, n574);
    let n576 = b.mk_union(n536, n575);
    let n577 = b.mk_union(n535, n576);
    let n578 = b.mk_union(n533, n577);
    let n579 = b.mk_union(n531, n578);
    let n580 = b.mk_union(n529, n579);
    let n581 = b.mk_union(n528, n580);
    let n582 = b.mk_union(n475, n581);
    let n583 = b.mk_concat(n526, n582);
    let n584 = b.mk_range_u8(0xF0, 0xF0);
    let n585 = b.mk_range_u8(0xA0, 0xBC);
    let n586 = b.mk_concat(n93, n585);
    let n587 = b.mk_range_u8(0x80, 0xBA);
    let n588 = b.mk_concat(n203, n587);
    let n589 = b.mk_ranges_u8(&[
        (0x80, 0x8B),
        (0x8D, 0xA6),
        (0xA8, 0xBA),
        (0xBC, 0xBD),
        (0xBF, 0xBF),
    ]);
    let n590 = b.mk_concat(n195, n589);
    let n591 = b.mk_ranges_u8(&[(0x80, 0x8D), (0x90, 0x9D)]);
    let n592 = b.mk_concat(n197, n591);
    let n593 = b.mk_concat(n200, n8);
    let n594 = b.mk_range_u8(0x80, 0xB4);
    let n595 = b.mk_concat(n360, n594);
    let n596 = b.mk_concat(n413, n153);
    let n597 = b.mk_ranges_u8(&[(0x80, 0x9C), (0xA0, 0xBF)]);
    let n598 = b.mk_concat(n211, n597);
    let n599 = b.mk_ranges_u8(&[(0x80, 0x90), (0xA0, 0xA0)]);
    let n600 = b.mk_concat(n214, n599);
    let n601 = b.mk_ranges_u8(&[(0x80, 0x9F), (0xAD, 0xBF)]);
    let n602 = b.mk_concat(n217, n601);
    let n603 = b.mk_ranges_u8(&[(0x80, 0x8A), (0x90, 0xBA)]);
    let n604 = b.mk_concat(n220, n603);
    let n605 = b.mk_concat(n223, n201);
    let n606 = b.mk_ranges_u8(&[(0x80, 0x83), (0x88, 0x8F), (0x91, 0x95)]);
    let n607 = b.mk_concat(n226, n606);
    let n608 = b.mk_range_u8(0x90, 0x91);
    let n609 = b.mk_concat(n608, n8);
    let n610 = b.mk_ranges_u8(&[(0x80, 0x9D), (0xA0, 0xA9), (0xB0, 0xBF)]);
    let n611 = b.mk_concat(n365, n610);
    let n612 = b.mk_ranges_u8(&[(0x80, 0x93), (0x98, 0xBB)]);
    let n613 = b.mk_concat(n368, n612);
    let n614 = b.mk_range_u8(0x94, 0x94);
    let n615 = b.mk_ranges_u8(&[(0x80, 0xA7), (0xB0, 0xBF)]);
    let n616 = b.mk_concat(n614, n615);
    let n617 = b.mk_range_u8(0x95, 0x95);
    let n618 = b.mk_ranges_u8(&[(0x80, 0xA3), (0xB0, 0xBA), (0xBC, 0xBF)]);
    let n619 = b.mk_concat(n617, n618);
    let n620 = b.mk_range_u8(0x96, 0x96);
    let n621 = b.mk_ranges_u8(&[
        (0x80, 0x8A),
        (0x8C, 0x92),
        (0x94, 0x95),
        (0x97, 0xA1),
        (0xA3, 0xB1),
        (0xB3, 0xB9),
        (0xBB, 0xBC),
    ]);
    let n622 = b.mk_concat(n620, n621);
    let n623 = b.mk_range_u8(0x97, 0x97);
    let n624 = b.mk_concat(n623, n282);
    let n625 = b.mk_range_u8(0x98, 0x9B);
    let n626 = b.mk_concat(n625, n8);
    let n627 = b.mk_concat(n243, n427);
    let n628 = b.mk_ranges_u8(&[(0x80, 0x95), (0xA0, 0xA7)]);
    let n629 = b.mk_concat(n246, n628);
    let n630 = b.mk_ranges_u8(&[(0x80, 0x85), (0x87, 0xB0), (0xB2, 0xBA)]);
    let n631 = b.mk_concat(n249, n630);
    let n632 = b.mk_ranges_u8(&[
        (0x80, 0x85),
        (0x88, 0x88),
        (0x8A, 0xB5),
        (0xB7, 0xB8),
        (0xBC, 0xBC),
        (0xBF, 0xBF),
    ]);
    let n633 = b.mk_concat(n67, n632);
    let n634 = b.mk_ranges_u8(&[(0x80, 0x95), (0xA0, 0xB6)]);
    let n635 = b.mk_concat(n70, n634);
    let n636 = b.mk_range_u8(0x80, 0x9E);
    let n637 = b.mk_concat(n73, n636);
    let n638 = b.mk_ranges_u8(&[(0xA0, 0xB2), (0xB4, 0xB5)]);
    let n639 = b.mk_concat(n76, n638);
    let n640 = b.mk_ranges_u8(&[(0x80, 0x95), (0xA0, 0xB9)]);
    let n641 = b.mk_concat(n79, n640);
    let n642 = b.mk_ranges_u8(&[(0x80, 0xB7), (0xBE, 0xBF)]);
    let n643 = b.mk_concat(n84, n642);
    let n644 = b.mk_ranges_u8(&[
        (0x80, 0x83),
        (0x85, 0x86),
        (0x8C, 0x93),
        (0x95, 0x97),
        (0x99, 0xB5),
        (0xB8, 0xBA),
        (0xBF, 0xBF),
    ]);
    let n645 = b.mk_concat(n90, n644);
    let n646 = b.mk_range_u8(0x80, 0x9C);
    let n647 = b.mk_concat(n96, n646);
    let n648 = b.mk_ranges_u8(&[(0x80, 0x87), (0x89, 0xA6)]);
    let n649 = b.mk_concat(n99, n648);
    let n650 = b.mk_concat(n102, n260);
    let n651 = b.mk_ranges_u8(&[(0x80, 0x95), (0xA0, 0xB2)]);
    let n652 = b.mk_concat(n105, n651);
    let n653 = b.mk_concat(n108, n437);
    let n654 = b.mk_concat(n114, n8);
    let n655 = b.mk_concat(n117, n363);
    let n656 = b.mk_range_u8(0x80, 0xB2);
    let n657 = b.mk_concat(n120, n656);
    let n658 = b.mk_concat(n123, n656);
    let n659 = b.mk_ranges_u8(&[(0x80, 0xA7), (0xB0, 0xB9)]);
    let n660 = b.mk_concat(n126, n659);
    let n661 = b.mk_ranges_u8(&[(0x80, 0xA5), (0xA9, 0xAD), (0xAF, 0xBF)]);
    let n662 = b.mk_concat(n129, n661);
    let n663 = b.mk_range_u8(0x80, 0x85);
    let n664 = b.mk_concat(n132, n663);
    let n665 = b.mk_ranges_u8(&[(0x80, 0xA9), (0xAB, 0xAC), (0xB0, 0xB1)]);
    let n666 = b.mk_concat(n144, n665);
    let n667 = b.mk_ranges_u8(&[(0x82, 0x84), (0xBC, 0xBF)]);
    let n668 = b.mk_concat(n147, n667);
    let n669 = b.mk_ranges_u8(&[(0x80, 0x9C), (0xA7, 0xA7), (0xB0, 0xBF)]);
    let n670 = b.mk_concat(n150, n669);
    let n671 = b.mk_ranges_u8(&[(0x80, 0x90), (0xB0, 0xBF)]);
    let n672 = b.mk_concat(n153, n671);
    let n673 = b.mk_ranges_u8(&[(0x80, 0x85), (0xB0, 0xBF)]);
    let n674 = b.mk_concat(n156, n673);
    let n675 = b.mk_ranges_u8(&[(0x80, 0x84), (0xA0, 0xB6)]);
    let n676 = b.mk_concat(n159, n675);
    let n677 = b.mk_union(n674, n676);
    let n678 = b.mk_union(n672, n677);
    let n679 = b.mk_union(n670, n678);
    let n680 = b.mk_union(n668, n679);
    let n681 = b.mk_union(n666, n680);
    let n682 = b.mk_union(n664, n681);
    let n683 = b.mk_union(n662, n682);
    let n684 = b.mk_union(n660, n683);
    let n685 = b.mk_union(n658, n684);
    let n686 = b.mk_union(n657, n685);
    let n687 = b.mk_union(n655, n686);
    let n688 = b.mk_union(n654, n687);
    let n689 = b.mk_union(n653, n688);
    let n690 = b.mk_union(n652, n689);
    let n691 = b.mk_union(n650, n690);
    let n692 = b.mk_union(n649, n691);
    let n693 = b.mk_union(n647, n692);
    let n694 = b.mk_union(n645, n693);
    let n695 = b.mk_union(n643, n694);
    let n696 = b.mk_union(n641, n695);
    let n697 = b.mk_union(n639, n696);
    let n698 = b.mk_union(n637, n697);
    let n699 = b.mk_union(n635, n698);
    let n700 = b.mk_union(n633, n699);
    let n701 = b.mk_union(n631, n700);
    let n702 = b.mk_union(n629, n701);
    let n703 = b.mk_union(n627, n702);
    let n704 = b.mk_union(n626, n703);
    let n705 = b.mk_union(n624, n704);
    let n706 = b.mk_union(n622, n705);
    let n707 = b.mk_union(n619, n706);
    let n708 = b.mk_union(n616, n707);
    let n709 = b.mk_union(n613, n708);
    let n710 = b.mk_union(n611, n709);
    let n711 = b.mk_union(n609, n710);
    let n712 = b.mk_union(n607, n711);
    let n713 = b.mk_union(n605, n712);
    let n714 = b.mk_union(n604, n713);
    let n715 = b.mk_union(n602, n714);
    let n716 = b.mk_union(n600, n715);
    let n717 = b.mk_union(n598, n716);
    let n718 = b.mk_union(n596, n717);
    let n719 = b.mk_union(n595, n718);
    let n720 = b.mk_union(n593, n719);
    let n721 = b.mk_union(n592, n720);
    let n722 = b.mk_union(n590, n721);
    let n723 = b.mk_union(n588, n722);
    let n724 = b.mk_union(n586, n723);
    let n725 = b.mk_concat(n229, n724);
    let n726 = b.mk_range_u8(0x91, 0x91);
    let n727 = b.mk_ranges_u8(&[(0x80, 0x86), (0xA6, 0xB5), (0xBF, 0xBF)]);
    let n728 = b.mk_concat(n197, n727);
    let n729 = b.mk_concat(n200, n587);
    let n730 = b.mk_ranges_u8(&[(0x82, 0x82), (0x90, 0xA8), (0xB0, 0xB9)]);
    let n731 = b.mk_concat(n203, n730);
    let n732 = b.mk_ranges_u8(&[(0x80, 0xB4), (0xB6, 0xBF)]);
    let n733 = b.mk_concat(n357, n732);
    let n734 = b.mk_ranges_u8(&[(0x84, 0x87), (0x90, 0xB3), (0xB6, 0xB6)]);
    let n735 = b.mk_concat(n360, n734);
    let n736 = b.mk_concat(n160, n8);
    let n737 = b.mk_ranges_u8(&[(0x80, 0x84), (0x89, 0x8C), (0x8E, 0x9A), (0x9C, 0x9C)]);
    let n738 = b.mk_concat(n413, n737);
    let n739 = b.mk_range_u8(0x88, 0x88);
    let n740 = b.mk_ranges_u8(&[(0x80, 0x91), (0x93, 0xB7), (0xBE, 0xBF)]);
    let n741 = b.mk_concat(n739, n740);
    let n742 = b.mk_range_u8(0x80, 0x81);
    let n743 = b.mk_concat(n208, n742);
    let n744 = b.mk_ranges_u8(&[
        (0x80, 0x86),
        (0x88, 0x88),
        (0x8A, 0x8D),
        (0x8F, 0x9D),
        (0x9F, 0xA8),
        (0xB0, 0xBF),
    ]);
    let n745 = b.mk_concat(n211, n744);
    let n746 = b.mk_ranges_u8(&[(0x80, 0xAA), (0xB0, 0xB9)]);
    let n747 = b.mk_concat(n214, n746);
    let n748 = b.mk_ranges_u8(&[
        (0x80, 0x83),
        (0x85, 0x8C),
        (0x8F, 0x90),
        (0x93, 0xA8),
        (0xAA, 0xB0),
        (0xB2, 0xB3),
        (0xB5, 0xB9),
        (0xBB, 0xBF),
    ]);
    let n749 = b.mk_concat(n217, n748);
    let n750 = b.mk_ranges_u8(&[
        (0x80, 0x84),
        (0x87, 0x88),
        (0x8B, 0x8D),
        (0x90, 0x90),
        (0x97, 0x97),
        (0x9D, 0xA3),
        (0xA6, 0xAC),
        (0xB0, 0xB4),
    ]);
    let n751 = b.mk_concat(n220, n750);
    let n752 = b.mk_ranges_u8(&[
        (0x80, 0x89),
        (0x8B, 0x8B),
        (0x8E, 0x8E),
        (0x90, 0xB5),
        (0xB7, 0xBF),
    ]);
    let n753 = b.mk_concat(n223, n752);
    let n754 = b.mk_ranges_u8(&[
        (0x80, 0x80),
        (0x82, 0x82),
        (0x85, 0x85),
        (0x87, 0x8A),
        (0x8C, 0x93),
        (0xA1, 0xA2),
    ]);
    let n755 = b.mk_concat(n226, n754);
    let n756 = b.mk_concat(n229, n8);
    let n757 = b.mk_ranges_u8(&[(0x80, 0x8A), (0x90, 0x99), (0x9E, 0xA1)]);
    let n758 = b.mk_concat(n726, n757);
    let n759 = b.mk_concat(n365, n8);
    let n760 = b.mk_ranges_u8(&[(0x80, 0x85), (0x87, 0x87), (0x90, 0x99)]);
    let n761 = b.mk_concat(n368, n760);
    let n762 = b.mk_ranges_u8(&[(0x80, 0xB5), (0xB8, 0xBF)]);
    let n763 = b.mk_concat(n620, n762);
    let n764 = b.mk_ranges_u8(&[(0x80, 0x80), (0x98, 0x9D)]);
    let n765 = b.mk_concat(n623, n764);
    let n766 = b.mk_concat(n445, n8);
    let n767 = b.mk_ranges_u8(&[(0x80, 0x80), (0x84, 0x84), (0x90, 0x99)]);
    let n768 = b.mk_concat(n234, n767);
    let n769 = b.mk_concat(n237, n256);
    let n770 = b.mk_ranges_u8(&[(0x80, 0x89), (0x90, 0xA3)]);
    let n771 = b.mk_concat(n240, n770);
    let n772 = b.mk_ranges_u8(&[(0x80, 0x9A), (0x9D, 0xAB), (0xB0, 0xB9)]);
    let n773 = b.mk_concat(n243, n772);
    let n774 = b.mk_range_u8(0x80, 0x86);
    let n775 = b.mk_concat(n246, n774);
    let n776 = b.mk_concat(n67, n587);
    let n777 = b.mk_range_u8(0xA0, 0xBF);
    let n778 = b.mk_concat(n73, n777);
    let n779 = b.mk_ranges_u8(&[(0x80, 0xA9), (0xBF, 0xBF)]);
    let n780 = b.mk_concat(n76, n779);
    let n781 = b.mk_ranges_u8(&[
        (0x80, 0x86),
        (0x89, 0x89),
        (0x8C, 0x93),
        (0x95, 0x96),
        (0x98, 0xB5),
        (0xB7, 0xB8),
        (0xBB, 0xBF),
    ]);
    let n782 = b.mk_concat(n79, n781);
    let n783 = b.mk_ranges_u8(&[(0x80, 0x83), (0x90, 0x99)]);
    let n784 = b.mk_concat(n81, n783);
    let n785 = b.mk_ranges_u8(&[(0xA0, 0xA7), (0xAA, 0xBF)]);
    let n786 = b.mk_concat(n84, n785);
    let n787 = b.mk_ranges_u8(&[(0x80, 0x97), (0x9A, 0xA1), (0xA3, 0xA4)]);
    let n788 = b.mk_concat(n87, n787);
    let n789 = b.mk_concat(n90, n559);
    let n790 = b.mk_ranges_u8(&[(0x87, 0x87), (0x90, 0xBF)]);
    let n791 = b.mk_concat(n93, n790);
    let n792 = b.mk_ranges_u8(&[(0x80, 0x99), (0x9D, 0x9D), (0xB0, 0xBF)]);
    let n793 = b.mk_concat(n96, n792);
    let n794 = b.mk_concat(n99, n256);
    let n795 = b.mk_ranges_u8(&[(0x80, 0xA0), (0xB0, 0xB9)]);
    let n796 = b.mk_concat(n111, n795);
    let n797 = b.mk_ranges_u8(&[(0x80, 0x88), (0x8A, 0xB6), (0xB8, 0xBF)]);
    let n798 = b.mk_concat(n114, n797);
    let n799 = b.mk_ranges_u8(&[(0x80, 0x80), (0x90, 0x99), (0xB2, 0xBF)]);
    let n800 = b.mk_concat(n117, n799);
    let n801 = b.mk_ranges_u8(&[(0x80, 0x8F), (0x92, 0xA7), (0xA9, 0xB6)]);
    let n802 = b.mk_concat(n120, n801);
    let n803 = b.mk_ranges_u8(&[
        (0x80, 0x86),
        (0x88, 0x89),
        (0x8B, 0xB6),
        (0xBA, 0xBA),
        (0xBC, 0xBD),
        (0xBF, 0xBF),
    ]);
    let n804 = b.mk_concat(n126, n803);
    let n805 = b.mk_ranges_u8(&[
        (0x80, 0x87),
        (0x90, 0x99),
        (0xA0, 0xA5),
        (0xA7, 0xA8),
        (0xAA, 0xBF),
    ]);
    let n806 = b.mk_concat(n129, n805);
    let n807 = b.mk_ranges_u8(&[(0x80, 0x8E), (0x90, 0x91), (0x93, 0x98), (0xA0, 0xA9)]);
    let n808 = b.mk_concat(n132, n807);
    let n809 = b.mk_range_u8(0xA0, 0xB6);
    let n810 = b.mk_concat(n147, n809);
    let n811 = b.mk_ranges_u8(&[(0x80, 0x90), (0x92, 0xBA), (0xBE, 0xBF)]);
    let n812 = b.mk_concat(n150, n811);
    let n813 = b.mk_ranges_u8(&[(0x80, 0x82), (0x90, 0x9A)]);
    let n814 = b.mk_concat(n153, n813);
    let n815 = b.mk_concat(n156, n114);
    let n816 = b.mk_union(n814, n815);
    let n817 = b.mk_union(n812, n816);
    let n818 = b.mk_union(n810, n817);
    let n819 = b.mk_union(n808, n818);
    let n820 = b.mk_union(n806, n819);
    let n821 = b.mk_union(n804, n820);
    let n822 = b.mk_union(n802, n821);
    let n823 = b.mk_union(n800, n822);
    let n824 = b.mk_union(n798, n823);
    let n825 = b.mk_union(n796, n824);
    let n826 = b.mk_union(n794, n825);
    let n827 = b.mk_union(n793, n826);
    let n828 = b.mk_union(n791, n827);
    let n829 = b.mk_union(n789, n828);
    let n830 = b.mk_union(n788, n829);
    let n831 = b.mk_union(n786, n830);
    let n832 = b.mk_union(n784, n831);
    let n833 = b.mk_union(n782, n832);
    let n834 = b.mk_union(n780, n833);
    let n835 = b.mk_union(n778, n834);
    let n836 = b.mk_union(n776, n835);
    let n837 = b.mk_union(n775, n836);
    let n838 = b.mk_union(n773, n837);
    let n839 = b.mk_union(n771, n838);
    let n840 = b.mk_union(n769, n839);
    let n841 = b.mk_union(n768, n840);
    let n842 = b.mk_union(n766, n841);
    let n843 = b.mk_union(n765, n842);
    let n844 = b.mk_union(n763, n843);
    let n845 = b.mk_union(n761, n844);
    let n846 = b.mk_union(n759, n845);
    let n847 = b.mk_union(n758, n846);
    let n848 = b.mk_union(n756, n847);
    let n849 = b.mk_union(n755, n848);
    let n850 = b.mk_union(n753, n849);
    let n851 = b.mk_union(n751, n850);
    let n852 = b.mk_union(n749, n851);
    let n853 = b.mk_union(n747, n852);
    let n854 = b.mk_union(n745, n853);
    let n855 = b.mk_union(n743, n854);
    let n856 = b.mk_union(n741, n855);
    let n857 = b.mk_union(n738, n856);
    let n858 = b.mk_union(n736, n857);
    let n859 = b.mk_union(n735, n858);
    let n860 = b.mk_union(n733, n859);
    let n861 = b.mk_union(n731, n860);
    let n862 = b.mk_union(n729, n861);
    let n863 = b.mk_union(n728, n862);
    let n864 = b.mk_union(n196, n863);
    let n865 = b.mk_concat(n726, n864);
    let n866 = b.mk_range_u8(0x80, 0x8D);
    let n867 = b.mk_concat(n866, n8);
    let n868 = b.mk_concat(n223, n530);
    let n869 = b.mk_range_u8(0x80, 0xAE);
    let n870 = b.mk_concat(n726, n869);
    let n871 = b.mk_range_u8(0x92, 0x94);
    let n872 = b.mk_concat(n871, n8);
    let n873 = b.mk_range_u8(0x80, 0x83);
    let n874 = b.mk_concat(n617, n873);
    let n875 = b.mk_concat(n156, n55);
    let n876 = b.mk_range_u8(0x80, 0xB0);
    let n877 = b.mk_concat(n159, n876);
    let n878 = b.mk_union(n875, n877);
    let n879 = b.mk_union(n874, n878);
    let n880 = b.mk_union(n872, n879);
    let n881 = b.mk_union(n870, n880);
    let n882 = b.mk_union(n868, n881);
    let n883 = b.mk_union(n867, n882);
    let n884 = b.mk_union(n756, n883);
    let n885 = b.mk_concat(n365, n884);
    let n886 = b.mk_range_u8(0x80, 0x8F);
    let n887 = b.mk_concat(n886, n8);
    let n888 = b.mk_range_u8(0x80, 0xAF);
    let n889 = b.mk_concat(n229, n888);
    let n890 = b.mk_ranges_u8(&[(0x80, 0x95), (0xA0, 0xBF)]);
    let n891 = b.mk_concat(n726, n890);
    let n892 = b.mk_range_u8(0x92, 0xBF);
    let n893 = b.mk_concat(n892, n8);
    let n894 = b.mk_union(n891, n893);
    let n895 = b.mk_union(n889, n894);
    let n896 = b.mk_union(n887, n895);
    let n897 = b.mk_concat(n368, n896);
    let n898 = b.mk_concat(n276, n8);
    let n899 = b.mk_concat(n226, n587);
    let n900 = b.mk_range_u8(0x90, 0x98);
    let n901 = b.mk_concat(n900, n8);
    let n902 = b.mk_concat(n234, n774);
    let n903 = b.mk_union(n901, n902);
    let n904 = b.mk_union(n899, n903);
    let n905 = b.mk_union(n898, n904);
    let n906 = b.mk_concat(n614, n905);
    let n907 = b.mk_range_u8(0x80, 0xB9);
    let n908 = b.mk_concat(n357, n907);
    let n909 = b.mk_range_u8(0xA0, 0xA7);
    let n910 = b.mk_concat(n909, n8);
    let n911 = b.mk_concat(n90, n256);
    let n912 = b.mk_ranges_u8(&[(0x80, 0x9E), (0xA0, 0xA9), (0xB0, 0xBF)]);
    let n913 = b.mk_concat(n93, n912);
    let n914 = b.mk_concat(n96, n559);
    let n915 = b.mk_ranges_u8(&[(0x80, 0x89), (0x90, 0xAD), (0xB0, 0xB4)]);
    let n916 = b.mk_concat(n99, n915);
    let n917 = b.mk_concat(n102, n427);
    let n918 = b.mk_ranges_u8(&[(0x80, 0x83), (0x90, 0x99), (0xA3, 0xB7), (0xBD, 0xBF)]);
    let n919 = b.mk_concat(n105, n918);
    let n920 = b.mk_concat(n108, n886);
    let n921 = b.mk_ranges_u8(&[(0x80, 0xAC), (0xB0, 0xB9)]);
    let n922 = b.mk_concat(n129, n921);
    let n923 = b.mk_concat(n141, n8);
    let n924 = b.mk_concat(n150, n8);
    let n925 = b.mk_ranges_u8(&[(0x80, 0x8A), (0x8F, 0xBF)]);
    let n926 = b.mk_concat(n153, n925);
    let n927 = b.mk_ranges_u8(&[(0x80, 0x87), (0x8F, 0x9F)]);
    let n928 = b.mk_concat(n156, n927);
    let n929 = b.mk_ranges_u8(&[(0xA0, 0xA1), (0xA3, 0xA4), (0xB0, 0xB1)]);
    let n930 = b.mk_concat(n159, n929);
    let n931 = b.mk_union(n928, n930);
    let n932 = b.mk_union(n926, n931);
    let n933 = b.mk_union(n924, n932);
    let n934 = b.mk_union(n923, n933);
    let n935 = b.mk_union(n922, n934);
    let n936 = b.mk_union(n920, n935);
    let n937 = b.mk_union(n919, n936);
    let n938 = b.mk_union(n917, n937);
    let n939 = b.mk_union(n916, n938);
    let n940 = b.mk_union(n914, n939);
    let n941 = b.mk_union(n913, n940);
    let n942 = b.mk_union(n911, n941);
    let n943 = b.mk_union(n910, n942);
    let n944 = b.mk_union(n908, n943);
    let n945 = b.mk_concat(n620, n944);
    let n946 = b.mk_concat(n623, n434);
    let n947 = b.mk_concat(n636, n8);
    let n948 = b.mk_concat(n251, n284);
    let n949 = b.mk_range_u8(0xA0, 0xB2);
    let n950 = b.mk_concat(n949, n8);
    let n951 = b.mk_ranges_u8(&[(0x80, 0x95), (0xBF, 0xBF)]);
    let n952 = b.mk_concat(n123, n951);
    let n953 = b.mk_concat(n126, n363);
    let n954 = b.mk_union(n952, n953);
    let n955 = b.mk_union(n950, n954);
    let n956 = b.mk_union(n948, n955);
    let n957 = b.mk_union(n947, n956);
    let n958 = b.mk_concat(n445, n957);
    let n959 = b.mk_ranges_u8(&[(0xB0, 0xB3), (0xB5, 0xBB), (0xBD, 0xBE)]);
    let n960 = b.mk_concat(n159, n959);
    let n961 = b.mk_concat(n237, n960);
    let n962 = b.mk_concat(n873, n8);
    let n963 = b.mk_ranges_u8(&[(0x80, 0xA2), (0xB2, 0xB2)]);
    let n964 = b.mk_concat(n357, n963);
    let n965 = b.mk_ranges_u8(&[(0x90, 0x92), (0x95, 0x95), (0xA4, 0xA7), (0xB0, 0xBF)]);
    let n966 = b.mk_concat(n360, n965);
    let n967 = b.mk_range_u8(0x86, 0x8A);
    let n968 = b.mk_concat(n967, n8);
    let n969 = b.mk_range_u8(0x80, 0xBB);
    let n970 = b.mk_concat(n214, n969);
    let n971 = b.mk_ranges_u8(&[(0x80, 0xAA), (0xB0, 0xBC)]);
    let n972 = b.mk_concat(n117, n971);
    let n973 = b.mk_ranges_u8(&[(0x80, 0x88), (0x90, 0x99), (0x9D, 0x9E)]);
    let n974 = b.mk_concat(n120, n973);
    let n975 = b.mk_union(n972, n974);
    let n976 = b.mk_union(n970, n975);
    let n977 = b.mk_union(n968, n976);
    let n978 = b.mk_union(n966, n977);
    let n979 = b.mk_union(n964, n978);
    let n980 = b.mk_union(n962, n979);
    let n981 = b.mk_union(n654, n980);
    let n982 = b.mk_concat(n240, n981);
    let n983 = b.mk_range_u8(0xB0, 0xB9);
    let n984 = b.mk_concat(n123, n983);
    let n985 = b.mk_concat(n150, n465);
    let n986 = b.mk_concat(n153, n774);
    let n987 = b.mk_union(n985, n986);
    let n988 = b.mk_union(n984, n987);
    let n989 = b.mk_concat(n243, n988);
    let n990 = b.mk_ranges_u8(&[(0xA5, 0xA9), (0xAD, 0xB2), (0xBB, 0xBF)]);
    let n991 = b.mk_concat(n360, n990);
    let n992 = b.mk_ranges_u8(&[(0x80, 0x82), (0x85, 0x8B), (0xAA, 0xAD)]);
    let n993 = b.mk_concat(n160, n992);
    let n994 = b.mk_range_u8(0x82, 0x84);
    let n995 = b.mk_concat(n208, n994);
    let n996 = b.mk_ranges_u8(&[(0x80, 0x94), (0x96, 0xBF)]);
    let n997 = b.mk_concat(n726, n996);
    let n998 = b.mk_ranges_u8(&[
        (0x80, 0x9C),
        (0x9E, 0x9F),
        (0xA2, 0xA2),
        (0xA5, 0xA6),
        (0xA9, 0xAC),
        (0xAE, 0xB9),
        (0xBB, 0xBB),
        (0xBD, 0xBF),
    ]);
    let n999 = b.mk_concat(n365, n998);
    let n1000 = b.mk_ranges_u8(&[(0x80, 0x83), (0x85, 0xBF)]);
    let n1001 = b.mk_concat(n368, n1000);
    let n1002 = b.mk_ranges_u8(&[
        (0x80, 0x85),
        (0x87, 0x8A),
        (0x8D, 0x94),
        (0x96, 0x9C),
        (0x9E, 0xB9),
        (0xBB, 0xBE),
    ]);
    let n1003 = b.mk_concat(n614, n1002);
    let n1004 = b.mk_ranges_u8(&[(0x80, 0x84), (0x86, 0x86), (0x8A, 0x90), (0x92, 0xBF)]);
    let n1005 = b.mk_concat(n617, n1004);
    let n1006 = b.mk_range_u8(0x96, 0x99);
    let n1007 = b.mk_concat(n1006, n8);
    let n1008 = b.mk_ranges_u8(&[(0x80, 0xA5), (0xA8, 0xBF)]);
    let n1009 = b.mk_concat(n237, n1008);
    let n1010 = b.mk_ranges_u8(&[(0x80, 0x80), (0x82, 0x9A), (0x9C, 0xBA), (0xBC, 0xBF)]);
    let n1011 = b.mk_concat(n240, n1010);
    let n1012 = b.mk_ranges_u8(&[(0x80, 0x94), (0x96, 0xB4), (0xB6, 0xBF)]);
    let n1013 = b.mk_concat(n243, n1012);
    let n1014 = b.mk_ranges_u8(&[(0x80, 0x8E), (0x90, 0xAE), (0xB0, 0xBF)]);
    let n1015 = b.mk_concat(n246, n1014);
    let n1016 = b.mk_ranges_u8(&[(0x80, 0x88), (0x8A, 0xA8), (0xAA, 0xBF)]);
    let n1017 = b.mk_concat(n249, n1016);
    let n1018 = b.mk_ranges_u8(&[(0x80, 0x82), (0x84, 0x8B), (0x8E, 0xBF)]);
    let n1019 = b.mk_concat(n251, n1018);
    let n1020 = b.mk_ranges_u8(&[(0x80, 0xB6), (0xBB, 0xBF)]);
    let n1021 = b.mk_concat(n90, n1020);
    let n1022 = b.mk_ranges_u8(&[(0x80, 0xAC), (0xB5, 0xB5)]);
    let n1023 = b.mk_concat(n93, n1022);
    let n1024 = b.mk_ranges_u8(&[(0x84, 0x84), (0x9B, 0x9F), (0xA1, 0xAF)]);
    let n1025 = b.mk_concat(n96, n1024);
    let n1026 = b.mk_ranges_u8(&[(0x80, 0x9E), (0xA5, 0xAA)]);
    let n1027 = b.mk_concat(n150, n1026);
    let n1028 = b.mk_union(n1025, n1027);
    let n1029 = b.mk_union(n1023, n1028);
    let n1030 = b.mk_union(n1021, n1029);
    let n1031 = b.mk_union(n1019, n1030);
    let n1032 = b.mk_union(n1017, n1031);
    let n1033 = b.mk_union(n1015, n1032);
    let n1034 = b.mk_union(n1013, n1033);
    let n1035 = b.mk_union(n1011, n1034);
    let n1036 = b.mk_union(n1009, n1035);
    let n1037 = b.mk_union(n1007, n1036);
    let n1038 = b.mk_union(n1005, n1037);
    let n1039 = b.mk_union(n1003, n1038);
    let n1040 = b.mk_union(n1001, n1039);
    let n1041 = b.mk_union(n999, n1040);
    let n1042 = b.mk_union(n997, n1041);
    let n1043 = b.mk_union(n995, n1042);
    let n1044 = b.mk_union(n993, n1043);
    let n1045 = b.mk_union(n991, n1044);
    let n1046 = b.mk_union(n756, n1045);
    let n1047 = b.mk_concat(n246, n1046);
    let n1048 = b.mk_ranges_u8(&[
        (0x80, 0x86),
        (0x88, 0x98),
        (0x9B, 0xA1),
        (0xA3, 0xA4),
        (0xA6, 0xAA),
        (0xB0, 0xBF),
    ]);
    let n1049 = b.mk_concat(n195, n1048);
    let n1050 = b.mk_concat(n197, n68);
    let n1051 = b.mk_concat(n200, n226);
    let n1052 = b.mk_ranges_u8(&[(0x80, 0xAC), (0xB0, 0xBD)]);
    let n1053 = b.mk_concat(n357, n1052);
    let n1054 = b.mk_ranges_u8(&[(0x80, 0x89), (0x8E, 0x8E)]);
    let n1055 = b.mk_concat(n360, n1054);
    let n1056 = b.mk_range_u8(0x90, 0xAE);
    let n1057 = b.mk_concat(n211, n1056);
    let n1058 = b.mk_concat(n214, n907);
    let n1059 = b.mk_range_u8(0x90, 0xB9);
    let n1060 = b.mk_concat(n368, n1059);
    let n1061 = b.mk_range_u8(0x90, 0xBA);
    let n1062 = b.mk_concat(n623, n1061);
    let n1063 = b.mk_ranges_u8(&[(0xA0, 0xA6), (0xA8, 0xAB), (0xAD, 0xAE), (0xB0, 0xBE)]);
    let n1064 = b.mk_concat(n251, n1063);
    let n1065 = b.mk_range_u8(0xA0, 0xA2);
    let n1066 = b.mk_concat(n1065, n8);
    let n1067 = b.mk_ranges_u8(&[(0x80, 0x84), (0x90, 0x96)]);
    let n1068 = b.mk_concat(n76, n1067);
    let n1069 = b.mk_ranges_u8(&[(0x80, 0x8B), (0x90, 0x99)]);
    let n1070 = b.mk_concat(n81, n1069);
    let n1071 = b.mk_ranges_u8(&[
        (0x80, 0x83),
        (0x85, 0x9F),
        (0xA1, 0xA2),
        (0xA4, 0xA4),
        (0xA7, 0xA7),
        (0xA9, 0xB2),
        (0xB4, 0xB7),
        (0xB9, 0xB9),
        (0xBB, 0xBB),
    ]);
    let n1072 = b.mk_concat(n138, n1071);
    let n1073 = b.mk_ranges_u8(&[
        (0x82, 0x82),
        (0x87, 0x87),
        (0x89, 0x89),
        (0x8B, 0x8B),
        (0x8D, 0x8F),
        (0x91, 0x92),
        (0x94, 0x94),
        (0x97, 0x97),
        (0x99, 0x99),
        (0x9B, 0x9B),
        (0x9D, 0x9D),
        (0x9F, 0x9F),
        (0xA1, 0xA2),
        (0xA4, 0xA4),
        (0xA7, 0xAA),
        (0xAC, 0xB2),
        (0xB4, 0xB7),
        (0xB9, 0xBC),
        (0xBE, 0xBE),
    ]);
    let n1074 = b.mk_concat(n141, n1073);
    let n1075 = b.mk_ranges_u8(&[
        (0x80, 0x89),
        (0x8B, 0x9B),
        (0xA1, 0xA3),
        (0xA5, 0xA9),
        (0xAB, 0xBB),
    ]);
    let n1076 = b.mk_concat(n144, n1075);
    let n1077 = b.mk_union(n1074, n1076);
    let n1078 = b.mk_union(n1072, n1077);
    let n1079 = b.mk_union(n1070, n1078);
    let n1080 = b.mk_union(n1068, n1079);
    let n1081 = b.mk_union(n1066, n1080);
    let n1082 = b.mk_union(n1064, n1081);
    let n1083 = b.mk_union(n1062, n1082);
    let n1084 = b.mk_union(n1060, n1083);
    let n1085 = b.mk_union(n1058, n1084);
    let n1086 = b.mk_union(n1057, n1085);
    let n1087 = b.mk_union(n1055, n1086);
    let n1088 = b.mk_union(n1053, n1087);
    let n1089 = b.mk_union(n1051, n1088);
    let n1090 = b.mk_union(n1050, n1089);
    let n1091 = b.mk_union(n1049, n1090);
    let n1092 = b.mk_union(n80, n1091);
    let n1093 = b.mk_concat(n249, n1092);
    let n1094 = b.mk_concat(n111, n983);
    let n1095 = b.mk_concat(n357, n414);
    let n1096 = b.mk_ranges_u8(&[(0x80, 0x89), (0x90, 0xA9), (0xB0, 0xBF)]);
    let n1097 = b.mk_concat(n360, n1096);
    let n1098 = b.mk_range_u8(0x80, 0x89);
    let n1099 = b.mk_concat(n160, n1098);
    let n1100 = b.mk_union(n1097, n1099);
    let n1101 = b.mk_union(n1095, n1100);
    let n1102 = b.mk_union(n1094, n1101);
    let n1103 = b.mk_concat(n251, n1102);
    let n1104 = b.mk_range_u8(0xA0, 0xA9);
    let n1105 = b.mk_concat(n1104, n434);
    let n1106 = b.mk_range_u8(0x80, 0x9A);
    let n1107 = b.mk_concat(n1106, n8);
    let n1108 = b.mk_range_u8(0x80, 0x9F);
    let n1109 = b.mk_concat(n240, n1108);
    let n1110 = b.mk_range_u8(0x9C, 0xBF);
    let n1111 = b.mk_concat(n1110, n8);
    let n1112 = b.mk_union(n1109, n1111);
    let n1113 = b.mk_union(n1107, n1112);
    let n1114 = b.mk_concat(n96, n1113);
    let n1115 = b.mk_range_u8(0x80, 0x9B);
    let n1116 = b.mk_concat(n1115, n8);
    let n1117 = b.mk_concat(n243, n907);
    let n1118 = b.mk_range_u8(0x9D, 0x9F);
    let n1119 = b.mk_concat(n1118, n8);
    let n1120 = b.mk_concat(n67, n201);
    let n1121 = b.mk_range_u8(0xA1, 0xBF);
    let n1122 = b.mk_concat(n1121, n8);
    let n1123 = b.mk_union(n1120, n1122);
    let n1124 = b.mk_union(n1119, n1123);
    let n1125 = b.mk_union(n1117, n1124);
    let n1126 = b.mk_union(n1116, n1125);
    let n1127 = b.mk_concat(n99, n1126);
    let n1128 = b.mk_concat(n907, n8);
    let n1129 = b.mk_ranges_u8(&[(0x80, 0xA1), (0xB0, 0xBF)]);
    let n1130 = b.mk_concat(n144, n1129);
    let n1131 = b.mk_range_u8(0xBB, 0xBF);
    let n1132 = b.mk_concat(n1131, n8);
    let n1133 = b.mk_union(n1130, n1132);
    let n1134 = b.mk_union(n1128, n1133);
    let n1135 = b.mk_concat(n102, n1134);
    let n1136 = b.mk_concat(n105, n434);
    let n1137 = b.mk_concat(n869, n8);
    let n1138 = b.mk_ranges_u8(&[(0x80, 0xA0), (0xB0, 0xBF)]);
    let n1139 = b.mk_concat(n111, n1138);
    let n1140 = b.mk_range_u8(0xB0, 0xB8);
    let n1141 = b.mk_concat(n1140, n8);
    let n1142 = b.mk_concat(n141, n517);
    let n1143 = b.mk_union(n1141, n1142);
    let n1144 = b.mk_union(n1139, n1143);
    let n1145 = b.mk_union(n1137, n1144);
    let n1146 = b.mk_concat(n108, n1145);
    let n1147 = b.mk_concat(n90, n517);
    let n1148 = b.mk_union(n910, n1147);
    let n1149 = b.mk_concat(n111, n1148);
    let n1150 = b.mk_concat(n114, n434);
    let n1151 = b.mk_concat(n439, n8);
    let n1152 = b.mk_ranges_u8(&[(0x80, 0x8A), (0x90, 0xBF)]);
    let n1153 = b.mk_concat(n220, n1152);
    let n1154 = b.mk_range_u8(0x8E, 0xBF);
    let n1155 = b.mk_concat(n1154, n8);
    let n1156 = b.mk_union(n1153, n1155);
    let n1157 = b.mk_union(n1151, n1156);
    let n1158 = b.mk_concat(n117, n1157);
    let n1159 = b.mk_concat(n223, n888);
    let n1160 = b.mk_union(n867, n1159);
    let n1161 = b.mk_concat(n120, n1160);
    let n1162 = b.mk_union(n1158, n1161);
    let n1163 = b.mk_union(n1150, n1162);
    let n1164 = b.mk_union(n1149, n1163);
    let n1165 = b.mk_union(n1146, n1164);
    let n1166 = b.mk_union(n1136, n1165);
    let n1167 = b.mk_union(n1135, n1166);
    let n1168 = b.mk_union(n1127, n1167);
    let n1169 = b.mk_union(n1114, n1168);
    let n1170 = b.mk_union(n1105, n1169);
    let n1171 = b.mk_union(n1103, n1170);
    let n1172 = b.mk_union(n1093, n1171);
    let n1173 = b.mk_union(n1047, n1172);
    let n1174 = b.mk_union(n989, n1173);
    let n1175 = b.mk_union(n982, n1174);
    let n1176 = b.mk_union(n961, n1175);
    let n1177 = b.mk_union(n958, n1176);
    let n1178 = b.mk_union(n946, n1177);
    let n1179 = b.mk_union(n945, n1178);
    let n1180 = b.mk_union(n906, n1179);
    let n1181 = b.mk_union(n897, n1180);
    let n1182 = b.mk_union(n885, n1181);
    let n1183 = b.mk_union(n865, n1182);
    let n1184 = b.mk_union(n725, n1183);
    let n1185 = b.mk_concat(n584, n1184);
    let n1186 = b.mk_range_u8(0xF3, 0xF3);
    let n1187 = b.mk_range_u8(0x84, 0x86);
    let n1188 = b.mk_concat(n1187, n8);
    let n1189 = b.mk_concat(n413, n888);
    let n1190 = b.mk_union(n1188, n1189);
    let n1191 = b.mk_concat(n67, n1190);
    let n1192 = b.mk_concat(n1186, n1191);
    let n1193 = b.mk_union(n1185, n1192);
    let n1194 = b.mk_union(n583, n1193);
    let n1195 = b.mk_union(n525, n1194);
    let n1196 = b.mk_union(n515, n1195);
    let n1197 = b.mk_union(n513, n1196);
    let n1198 = b.mk_union(n435, n1197);
    let n1199 = b.mk_union(n432, n1198);
    let n1200 = b.mk_union(n425, n1199);
    let n1201 = b.mk_union(n399, n1200);
    let n1202 = b.mk_union(n347, n1201);
    let n1203 = b.mk_union(n193, n1202);
    let n1204 = b.mk_union(n65, n1203);
    let n1205 = b.mk_union(n62, n1204);
    let n1206 = b.mk_union(n59, n1205);
    let n1207 = b.mk_union(n56, n1206);
    let n1208 = b.mk_union(n53, n1207);
    let n1209 = b.mk_union(n50, n1208);
    let n1210 = b.mk_union(n48, n1209);
    let n1211 = b.mk_union(n45, n1210);
    let n1212 = b.mk_union(n42, n1211);
    let n1213 = b.mk_union(n39, n1212);
    let n1214 = b.mk_union(n36, n1213);
    let n1215 = b.mk_union(n33, n1214);
    let n1216 = b.mk_union(n30, n1215);
    let n1217 = b.mk_union(n28, n1216);
    let n1218 = b.mk_union(n25, n1217);
    let n1219 = b.mk_union(n23, n1218);
    let n1220 = b.mk_union(n20, n1219);
    let n1221 = b.mk_union(n17, n1220);
    let n1222 = b.mk_union(n14, n1221);
    let n1223 = b.mk_union(n12, n1222);
    let n1224 = b.mk_union(n9, n1223);
    let n1225 = b.mk_union(n6, n1224);
    let n1226 = b.mk_union(n3, n1225);
    let n1227 = b.mk_union(n0, n1226);
    n1227
}

pub fn build_digit_class_full(b: &mut RegexBuilder) -> NodeId {
    let n0 = b.mk_range_u8(0x30, 0x39);
    let n1 = b.mk_range_u8(0xD9, 0xD9);
    let n2 = b.mk_range_u8(0xA0, 0xA9);
    let n3 = b.mk_concat(n1, n2);
    let n4 = b.mk_range_u8(0xDB, 0xDB);
    let n5 = b.mk_range_u8(0xB0, 0xB9);
    let n6 = b.mk_concat(n4, n5);
    let n7 = b.mk_range_u8(0xDF, 0xDF);
    let n8 = b.mk_range_u8(0x80, 0x89);
    let n9 = b.mk_concat(n7, n8);
    let n10 = b.mk_range_u8(0xE0, 0xE0);
    let n11 = b.mk_range_u8(0xA5, 0xA5);
    let n12 = b.mk_range_u8(0xA6, 0xAF);
    let n13 = b.mk_concat(n11, n12);
    let n14 = b.mk_range_u8(0xA7, 0xA7);
    let n15 = b.mk_concat(n14, n12);
    let n16 = b.mk_range_u8(0xA9, 0xA9);
    let n17 = b.mk_concat(n16, n12);
    let n18 = b.mk_range_u8(0xAB, 0xAB);
    let n19 = b.mk_concat(n18, n12);
    let n20 = b.mk_range_u8(0xAD, 0xAD);
    let n21 = b.mk_concat(n20, n12);
    let n22 = b.mk_range_u8(0xAF, 0xAF);
    let n23 = b.mk_concat(n22, n12);
    let n24 = b.mk_range_u8(0xB1, 0xB1);
    let n25 = b.mk_concat(n24, n12);
    let n26 = b.mk_range_u8(0xB3, 0xB3);
    let n27 = b.mk_concat(n26, n12);
    let n28 = b.mk_range_u8(0xB5, 0xB5);
    let n29 = b.mk_concat(n28, n12);
    let n30 = b.mk_range_u8(0xB7, 0xB7);
    let n31 = b.mk_concat(n30, n12);
    let n32 = b.mk_range_u8(0xB9, 0xB9);
    let n33 = b.mk_range_u8(0x90, 0x99);
    let n34 = b.mk_concat(n32, n33);
    let n35 = b.mk_range_u8(0xBB, 0xBB);
    let n36 = b.mk_concat(n35, n33);
    let n37 = b.mk_range_u8(0xBC, 0xBC);
    let n38 = b.mk_concat(n37, n2);
    let n39 = b.mk_union(n36, n38);
    let n40 = b.mk_union(n34, n39);
    let n41 = b.mk_union(n31, n40);
    let n42 = b.mk_union(n29, n41);
    let n43 = b.mk_union(n27, n42);
    let n44 = b.mk_union(n25, n43);
    let n45 = b.mk_union(n23, n44);
    let n46 = b.mk_union(n21, n45);
    let n47 = b.mk_union(n19, n46);
    let n48 = b.mk_union(n17, n47);
    let n49 = b.mk_union(n15, n48);
    let n50 = b.mk_union(n13, n49);
    let n51 = b.mk_concat(n10, n50);
    let n52 = b.mk_range_u8(0xE1, 0xE1);
    let n53 = b.mk_range_u8(0x81, 0x81);
    let n54 = b.mk_concat(n53, n8);
    let n55 = b.mk_range_u8(0x82, 0x82);
    let n56 = b.mk_concat(n55, n33);
    let n57 = b.mk_range_u8(0x9F, 0x9F);
    let n58 = b.mk_concat(n57, n2);
    let n59 = b.mk_range_u8(0xA0, 0xA0);
    let n60 = b.mk_concat(n59, n33);
    let n61 = b.mk_range_u8(0x86, 0x8F);
    let n62 = b.mk_concat(n11, n61);
    let n63 = b.mk_concat(n14, n33);
    let n64 = b.mk_range_u8(0xAA, 0xAA);
    let n65 = b.mk_ranges_u8(&[(0x80, 0x89), (0x90, 0x99)]);
    let n66 = b.mk_concat(n64, n65);
    let n67 = b.mk_concat(n20, n33);
    let n68 = b.mk_range_u8(0xAE, 0xAE);
    let n69 = b.mk_concat(n68, n5);
    let n70 = b.mk_concat(n24, n65);
    let n71 = b.mk_union(n69, n70);
    let n72 = b.mk_union(n67, n71);
    let n73 = b.mk_union(n66, n72);
    let n74 = b.mk_union(n63, n73);
    let n75 = b.mk_union(n62, n74);
    let n76 = b.mk_union(n60, n75);
    let n77 = b.mk_union(n58, n76);
    let n78 = b.mk_union(n56, n77);
    let n79 = b.mk_union(n54, n78);
    let n80 = b.mk_concat(n52, n79);
    let n81 = b.mk_range_u8(0xEA, 0xEA);
    let n82 = b.mk_range_u8(0x98, 0x98);
    let n83 = b.mk_concat(n82, n2);
    let n84 = b.mk_range_u8(0xA3, 0xA3);
    let n85 = b.mk_concat(n84, n33);
    let n86 = b.mk_range_u8(0xA4, 0xA4);
    let n87 = b.mk_concat(n86, n8);
    let n88 = b.mk_ranges_u8(&[(0x90, 0x99), (0xB0, 0xB9)]);
    let n89 = b.mk_concat(n14, n88);
    let n90 = b.mk_concat(n16, n33);
    let n91 = b.mk_concat(n22, n5);
    let n92 = b.mk_union(n90, n91);
    let n93 = b.mk_union(n89, n92);
    let n94 = b.mk_union(n87, n93);
    let n95 = b.mk_union(n85, n94);
    let n96 = b.mk_union(n83, n95);
    let n97 = b.mk_concat(n81, n96);
    let n98 = b.mk_range_u8(0xEF, 0xEF);
    let n99 = b.mk_concat(n37, n33);
    let n100 = b.mk_concat(n98, n99);
    let n101 = b.mk_range_u8(0xF0, 0xF0);
    let n102 = b.mk_range_u8(0x90, 0x90);
    let n103 = b.mk_range_u8(0x92, 0x92);
    let n104 = b.mk_concat(n103, n2);
    let n105 = b.mk_range_u8(0xB4, 0xB4);
    let n106 = b.mk_concat(n105, n5);
    let n107 = b.mk_concat(n28, n8);
    let n108 = b.mk_union(n106, n107);
    let n109 = b.mk_union(n104, n108);
    let n110 = b.mk_concat(n102, n109);
    let n111 = b.mk_range_u8(0x91, 0x91);
    let n112 = b.mk_concat(n24, n33);
    let n113 = b.mk_concat(n53, n12);
    let n114 = b.mk_range_u8(0x83, 0x83);
    let n115 = b.mk_concat(n114, n5);
    let n116 = b.mk_range_u8(0x84, 0x84);
    let n117 = b.mk_range_u8(0xB6, 0xBF);
    let n118 = b.mk_concat(n116, n117);
    let n119 = b.mk_range_u8(0x87, 0x87);
    let n120 = b.mk_concat(n119, n33);
    let n121 = b.mk_range_u8(0x8B, 0x8B);
    let n122 = b.mk_concat(n121, n5);
    let n123 = b.mk_concat(n111, n33);
    let n124 = b.mk_range_u8(0x93, 0x93);
    let n125 = b.mk_concat(n124, n33);
    let n126 = b.mk_range_u8(0x99, 0x99);
    let n127 = b.mk_concat(n126, n33);
    let n128 = b.mk_range_u8(0x9B, 0x9B);
    let n129 = b.mk_ranges_u8(&[(0x80, 0x89), (0x90, 0xA3)]);
    let n130 = b.mk_concat(n128, n129);
    let n131 = b.mk_range_u8(0x9C, 0x9C);
    let n132 = b.mk_concat(n131, n5);
    let n133 = b.mk_concat(n84, n2);
    let n134 = b.mk_concat(n11, n33);
    let n135 = b.mk_concat(n28, n33);
    let n136 = b.mk_range_u8(0xB6, 0xB6);
    let n137 = b.mk_concat(n136, n2);
    let n138 = b.mk_range_u8(0xBD, 0xBD);
    let n139 = b.mk_concat(n138, n33);
    let n140 = b.mk_union(n137, n139);
    let n141 = b.mk_union(n135, n140);
    let n142 = b.mk_union(n134, n141);
    let n143 = b.mk_union(n133, n142);
    let n144 = b.mk_union(n132, n143);
    let n145 = b.mk_union(n130, n144);
    let n146 = b.mk_union(n127, n145);
    let n147 = b.mk_union(n125, n146);
    let n148 = b.mk_union(n123, n147);
    let n149 = b.mk_union(n122, n148);
    let n150 = b.mk_union(n120, n149);
    let n151 = b.mk_union(n118, n150);
    let n152 = b.mk_union(n115, n151);
    let n153 = b.mk_union(n113, n152);
    let n154 = b.mk_union(n91, n153);
    let n155 = b.mk_union(n112, n154);
    let n156 = b.mk_concat(n111, n155);
    let n157 = b.mk_range_u8(0x96, 0x96);
    let n158 = b.mk_concat(n116, n5);
    let n159 = b.mk_concat(n16, n2);
    let n160 = b.mk_concat(n18, n8);
    let n161 = b.mk_concat(n28, n5);
    let n162 = b.mk_union(n160, n161);
    let n163 = b.mk_union(n159, n162);
    let n164 = b.mk_union(n158, n163);
    let n165 = b.mk_union(n67, n164);
    let n166 = b.mk_concat(n157, n165);
    let n167 = b.mk_concat(n26, n5);
    let n168 = b.mk_concat(n131, n167);
    let n169 = b.mk_range_u8(0x9D, 0x9D);
    let n170 = b.mk_range_u8(0x8E, 0xBF);
    let n171 = b.mk_concat(n57, n170);
    let n172 = b.mk_concat(n169, n171);
    let n173 = b.mk_range_u8(0x9E, 0x9E);
    let n174 = b.mk_range_u8(0x85, 0x85);
    let n175 = b.mk_concat(n174, n8);
    let n176 = b.mk_concat(n124, n5);
    let n177 = b.mk_range_u8(0x97, 0x97);
    let n178 = b.mk_range_u8(0xB1, 0xBA);
    let n179 = b.mk_concat(n177, n178);
    let n180 = b.mk_union(n176, n179);
    let n181 = b.mk_union(n175, n180);
    let n182 = b.mk_union(n134, n181);
    let n183 = b.mk_union(n122, n182);
    let n184 = b.mk_concat(n173, n183);
    let n185 = b.mk_concat(n57, n91);
    let n186 = b.mk_union(n184, n185);
    let n187 = b.mk_union(n172, n186);
    let n188 = b.mk_union(n168, n187);
    let n189 = b.mk_union(n166, n188);
    let n190 = b.mk_union(n156, n189);
    let n191 = b.mk_union(n110, n190);
    let n192 = b.mk_concat(n101, n191);
    let n193 = b.mk_union(n100, n192);
    let n194 = b.mk_union(n97, n193);
    let n195 = b.mk_union(n80, n194);
    let n196 = b.mk_union(n51, n195);
    let n197 = b.mk_union(n9, n196);
    let n198 = b.mk_union(n6, n197);
    let n199 = b.mk_union(n3, n198);
    let n200 = b.mk_union(n0, n199);
    n200
}
