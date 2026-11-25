use crate::{NodeId, RegexBuilder};

pub fn build_word_class(b: &mut RegexBuilder) -> NodeId {
    let n0 = b.mk_ranges_u8(&[(0x30, 0x39), (0x41, 0x5A), (0x5F, 0x5F), (0x61, 0x7A)]);
    let n1 = b.mk_range_u8(0xC2, 0xC2);
    let n2 = b.mk_ranges_u8(&[(0xAA, 0xAA), (0xB5, 0xB5), (0xBA, 0xBA)]);
    let n3 = b.mk_concat(n1, n2);
    let n4 = b.mk_range_u8(0xC3, 0xC3);
    let n5 = b.mk_ranges_u8(&[(0x80, 0x96), (0x98, 0xB6), (0xB8, 0xBF)]);
    let n6 = b.mk_concat(n4, n5);
    let n7 = b.mk_ranges_u8(&[
        (0xC4, 0xCA),
        (0xCC, 0xCC),
        (0xD0, 0xD1),
        (0xD3, 0xD3),
        (0xDA, 0xDA),
    ]);
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
    let n13 = b.mk_range_u8(0xCD, 0xCD);
    let n14 = b.mk_ranges_u8(&[(0x80, 0xB4), (0xB6, 0xB7), (0xBA, 0xBD), (0xBF, 0xBF)]);
    let n15 = b.mk_concat(n13, n14);
    let n16 = b.mk_range_u8(0xCE, 0xCE);
    let n17 = b.mk_ranges_u8(&[
        (0x86, 0x86),
        (0x88, 0x8A),
        (0x8C, 0x8C),
        (0x8E, 0xA1),
        (0xA3, 0xBF),
    ]);
    let n18 = b.mk_concat(n16, n17);
    let n19 = b.mk_range_u8(0xCF, 0xCF);
    let n20 = b.mk_ranges_u8(&[(0x80, 0xB5), (0xB7, 0xBF)]);
    let n21 = b.mk_concat(n19, n20);
    let n22 = b.mk_range_u8(0xD2, 0xD2);
    let n23 = b.mk_ranges_u8(&[(0x80, 0x81), (0x83, 0xBF)]);
    let n24 = b.mk_concat(n22, n23);
    let n25 = b.mk_range_u8(0xD4, 0xD4);
    let n26 = b.mk_ranges_u8(&[(0x80, 0xAF), (0xB1, 0xBF)]);
    let n27 = b.mk_concat(n25, n26);
    let n28 = b.mk_range_u8(0xD5, 0xD5);
    let n29 = b.mk_ranges_u8(&[(0x80, 0x96), (0x99, 0x99), (0xA0, 0xBF)]);
    let n30 = b.mk_concat(n28, n29);
    let n31 = b.mk_range_u8(0xD6, 0xD6);
    let n32 = b.mk_ranges_u8(&[(0x80, 0x88), (0x91, 0xBD), (0xBF, 0xBF)]);
    let n33 = b.mk_concat(n31, n32);
    let n34 = b.mk_range_u8(0xD7, 0xD7);
    let n35 = b.mk_ranges_u8(&[
        (0x81, 0x82),
        (0x84, 0x85),
        (0x87, 0x87),
        (0x90, 0xAA),
        (0xAF, 0xB2),
    ]);
    let n36 = b.mk_concat(n34, n35);
    let n37 = b.mk_range_u8(0xD8, 0xD8);
    let n38 = b.mk_ranges_u8(&[(0x90, 0x9A), (0xA0, 0xBF)]);
    let n39 = b.mk_concat(n37, n38);
    let n40 = b.mk_range_u8(0xD9, 0xD9);
    let n41 = b.mk_ranges_u8(&[(0x80, 0xA9), (0xAE, 0xBF)]);
    let n42 = b.mk_concat(n40, n41);
    let n43 = b.mk_range_u8(0xDB, 0xDB);
    let n44 = b.mk_ranges_u8(&[
        (0x80, 0x93),
        (0x95, 0x9C),
        (0x9F, 0xA8),
        (0xAA, 0xBC),
        (0xBF, 0xBF),
    ]);
    let n45 = b.mk_concat(n43, n44);
    let n46 = b.mk_range_u8(0xDC, 0xDC);
    let n47 = b.mk_range_u8(0x90, 0xBF);
    let n48 = b.mk_concat(n46, n47);
    let n49 = b.mk_range_u8(0xDD, 0xDD);
    let n50 = b.mk_ranges_u8(&[(0x80, 0x8A), (0x8D, 0xBF)]);
    let n51 = b.mk_concat(n49, n50);
    let n52 = b.mk_range_u8(0xDE, 0xDE);
    let n53 = b.mk_range_u8(0x80, 0xB1);
    let n54 = b.mk_concat(n52, n53);
    let n55 = b.mk_range_u8(0xDF, 0xDF);
    let n56 = b.mk_ranges_u8(&[(0x80, 0xB5), (0xBA, 0xBA), (0xBD, 0xBD)]);
    let n57 = b.mk_concat(n55, n56);
    let n58 = b.mk_union(n54, n57);
    let n59 = b.mk_union(n51, n58);
    let n60 = b.mk_union(n48, n59);
    let n61 = b.mk_union(n45, n60);
    let n62 = b.mk_union(n42, n61);
    let n63 = b.mk_union(n39, n62);
    let n64 = b.mk_union(n36, n63);
    let n65 = b.mk_union(n33, n64);
    let n66 = b.mk_union(n30, n65);
    let n67 = b.mk_union(n27, n66);
    let n68 = b.mk_union(n24, n67);
    let n69 = b.mk_union(n21, n68);
    let n70 = b.mk_union(n18, n69);
    let n71 = b.mk_union(n15, n70);
    let n72 = b.mk_union(n12, n71);
    let n73 = b.mk_union(n9, n72);
    let n74 = b.mk_union(n6, n73);
    let n75 = b.mk_union(n3, n74);
    let n76 = b.mk_union(n0, n75);
    n76
}

pub fn build_digit_class(b: &mut RegexBuilder) -> NodeId {
    // default \d to ascii [0-9], \p{Number} covers the full Unicode set of digits
    let n0 = b.mk_range_u8(0x30, 0x39);
    n0
}

pub fn build_space_class(b: &mut RegexBuilder) -> NodeId {
    // default \s to ascii [\t\n\v\f\r ], \p{White_Space} covers the full Unicode set
    let n0 = b.mk_ranges_u8(&[(0x09, 0x0D), (0x20, 0x20)]);
    n0
}

pub fn build_word_class_full(b: &mut RegexBuilder) -> NodeId {
    let n0 = b.mk_ranges_u8(&[(0x30, 0x39), (0x41, 0x5A), (0x5F, 0x5F), (0x61, 0x7A)]);
    let n1 = b.mk_range_u8(0xC2, 0xC2);
    let n2 = b.mk_ranges_u8(&[(0xAA, 0xAA), (0xB5, 0xB5), (0xBA, 0xBA)]);
    let n3 = b.mk_concat(n1, n2);
    let n4 = b.mk_range_u8(0xC3, 0xC3);
    let n5 = b.mk_ranges_u8(&[(0x80, 0x96), (0x98, 0xB6), (0xB8, 0xBF)]);
    let n6 = b.mk_concat(n4, n5);
    let n7 = b.mk_ranges_u8(&[
        (0xC4, 0xCA),
        (0xCC, 0xCC),
        (0xD0, 0xD1),
        (0xD3, 0xD3),
        (0xDA, 0xDA),
    ]);
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
    let n13 = b.mk_range_u8(0xCD, 0xCD);
    let n14 = b.mk_ranges_u8(&[(0x80, 0xB4), (0xB6, 0xB7), (0xBA, 0xBD), (0xBF, 0xBF)]);
    let n15 = b.mk_concat(n13, n14);
    let n16 = b.mk_range_u8(0xCE, 0xCE);
    let n17 = b.mk_ranges_u8(&[
        (0x86, 0x86),
        (0x88, 0x8A),
        (0x8C, 0x8C),
        (0x8E, 0xA1),
        (0xA3, 0xBF),
    ]);
    let n18 = b.mk_concat(n16, n17);
    let n19 = b.mk_range_u8(0xCF, 0xCF);
    let n20 = b.mk_ranges_u8(&[(0x80, 0xB5), (0xB7, 0xBF)]);
    let n21 = b.mk_concat(n19, n20);
    let n22 = b.mk_range_u8(0xD2, 0xD2);
    let n23 = b.mk_ranges_u8(&[(0x80, 0x81), (0x83, 0xBF)]);
    let n24 = b.mk_concat(n22, n23);
    let n25 = b.mk_range_u8(0xD4, 0xD4);
    let n26 = b.mk_ranges_u8(&[(0x80, 0xAF), (0xB1, 0xBF)]);
    let n27 = b.mk_concat(n25, n26);
    let n28 = b.mk_range_u8(0xD5, 0xD5);
    let n29 = b.mk_ranges_u8(&[(0x80, 0x96), (0x99, 0x99), (0xA0, 0xBF)]);
    let n30 = b.mk_concat(n28, n29);
    let n31 = b.mk_range_u8(0xD6, 0xD6);
    let n32 = b.mk_ranges_u8(&[(0x80, 0x88), (0x91, 0xBD), (0xBF, 0xBF)]);
    let n33 = b.mk_concat(n31, n32);
    let n34 = b.mk_range_u8(0xD7, 0xD7);
    let n35 = b.mk_ranges_u8(&[
        (0x81, 0x82),
        (0x84, 0x85),
        (0x87, 0x87),
        (0x90, 0xAA),
        (0xAF, 0xB2),
    ]);
    let n36 = b.mk_concat(n34, n35);
    let n37 = b.mk_range_u8(0xD8, 0xD8);
    let n38 = b.mk_ranges_u8(&[(0x90, 0x9A), (0xA0, 0xBF)]);
    let n39 = b.mk_concat(n37, n38);
    let n40 = b.mk_range_u8(0xD9, 0xD9);
    let n41 = b.mk_ranges_u8(&[(0x80, 0xA9), (0xAE, 0xBF)]);
    let n42 = b.mk_concat(n40, n41);
    let n43 = b.mk_range_u8(0xDB, 0xDB);
    let n44 = b.mk_ranges_u8(&[
        (0x80, 0x93),
        (0x95, 0x9C),
        (0x9F, 0xA8),
        (0xAA, 0xBC),
        (0xBF, 0xBF),
    ]);
    let n45 = b.mk_concat(n43, n44);
    let n46 = b.mk_range_u8(0xDC, 0xDC);
    let n47 = b.mk_range_u8(0x90, 0xBF);
    let n48 = b.mk_concat(n46, n47);
    let n49 = b.mk_range_u8(0xDD, 0xDD);
    let n50 = b.mk_ranges_u8(&[(0x80, 0x8A), (0x8D, 0xBF)]);
    let n51 = b.mk_concat(n49, n50);
    let n52 = b.mk_range_u8(0xDE, 0xDE);
    let n53 = b.mk_range_u8(0x80, 0xB1);
    let n54 = b.mk_concat(n52, n53);
    let n55 = b.mk_range_u8(0xDF, 0xDF);
    let n56 = b.mk_ranges_u8(&[(0x80, 0xB5), (0xBA, 0xBA), (0xBD, 0xBD)]);
    let n57 = b.mk_concat(n55, n56);
    let n58 = b.mk_range_u8(0xE0, 0xE0);
    let n59 = b.mk_range_u8(0xA0, 0xA0);
    let n60 = b.mk_range_u8(0x80, 0xAD);
    let n61 = b.mk_concat(n59, n60);
    let n62 = b.mk_range_u8(0xA1, 0xA1);
    let n63 = b.mk_ranges_u8(&[(0x80, 0x9B), (0xA0, 0xAA), (0xB0, 0xBF)]);
    let n64 = b.mk_concat(n62, n63);
    let n65 = b.mk_range_u8(0xA2, 0xA2);
    let n66 = b.mk_ranges_u8(&[(0x80, 0x87), (0x89, 0x8E), (0x97, 0xBF)]);
    let n67 = b.mk_concat(n65, n66);
    let n68 = b.mk_range_u8(0xA3, 0xA3);
    let n69 = b.mk_ranges_u8(&[(0x80, 0xA1), (0xA3, 0xBF)]);
    let n70 = b.mk_concat(n68, n69);
    let n71 = b.mk_range_u8(0xA4, 0xA4);
    let n72 = b.mk_concat(n71, n8);
    let n73 = b.mk_range_u8(0xA5, 0xA5);
    let n74 = b.mk_ranges_u8(&[(0x80, 0xA3), (0xA6, 0xAF), (0xB1, 0xBF)]);
    let n75 = b.mk_concat(n73, n74);
    let n76 = b.mk_range_u8(0xA6, 0xA6);
    let n77 = b.mk_ranges_u8(&[
        (0x80, 0x83),
        (0x85, 0x8C),
        (0x8F, 0x90),
        (0x93, 0xA8),
        (0xAA, 0xB0),
        (0xB2, 0xB2),
        (0xB6, 0xB9),
        (0xBC, 0xBF),
    ]);
    let n78 = b.mk_concat(n76, n77);
    let n79 = b.mk_range_u8(0xA7, 0xA7);
    let n80 = b.mk_ranges_u8(&[
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
    let n81 = b.mk_concat(n79, n80);
    let n82 = b.mk_range_u8(0xA8, 0xA8);
    let n83 = b.mk_ranges_u8(&[
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
    let n84 = b.mk_concat(n82, n83);
    let n85 = b.mk_range_u8(0xA9, 0xA9);
    let n86 = b.mk_ranges_u8(&[
        (0x80, 0x82),
        (0x87, 0x88),
        (0x8B, 0x8D),
        (0x91, 0x91),
        (0x99, 0x9C),
        (0x9E, 0x9E),
        (0xA6, 0xB5),
    ]);
    let n87 = b.mk_concat(n85, n86);
    let n88 = b.mk_range_u8(0xAA, 0xAA);
    let n89 = b.mk_ranges_u8(&[
        (0x81, 0x83),
        (0x85, 0x8D),
        (0x8F, 0x91),
        (0x93, 0xA8),
        (0xAA, 0xB0),
        (0xB2, 0xB3),
        (0xB5, 0xB9),
        (0xBC, 0xBF),
    ]);
    let n90 = b.mk_concat(n88, n89);
    let n91 = b.mk_range_u8(0xAB, 0xAB);
    let n92 = b.mk_ranges_u8(&[
        (0x80, 0x85),
        (0x87, 0x89),
        (0x8B, 0x8D),
        (0x90, 0x90),
        (0xA0, 0xA3),
        (0xA6, 0xAF),
        (0xB9, 0xBF),
    ]);
    let n93 = b.mk_concat(n91, n92);
    let n94 = b.mk_range_u8(0xAC, 0xAC);
    let n95 = b.mk_ranges_u8(&[
        (0x81, 0x83),
        (0x85, 0x8C),
        (0x8F, 0x90),
        (0x93, 0xA8),
        (0xAA, 0xB0),
        (0xB2, 0xB3),
        (0xB5, 0xB9),
        (0xBC, 0xBF),
    ]);
    let n96 = b.mk_concat(n94, n95);
    let n97 = b.mk_range_u8(0xAD, 0xAD);
    let n98 = b.mk_ranges_u8(&[
        (0x80, 0x84),
        (0x87, 0x88),
        (0x8B, 0x8D),
        (0x95, 0x97),
        (0x9C, 0x9D),
        (0x9F, 0xA3),
        (0xA6, 0xAF),
        (0xB1, 0xB1),
    ]);
    let n99 = b.mk_concat(n97, n98);
    let n100 = b.mk_range_u8(0xAE, 0xAE);
    let n101 = b.mk_ranges_u8(&[
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
    let n102 = b.mk_concat(n100, n101);
    let n103 = b.mk_range_u8(0xAF, 0xAF);
    let n104 = b.mk_ranges_u8(&[
        (0x80, 0x82),
        (0x86, 0x88),
        (0x8A, 0x8D),
        (0x90, 0x90),
        (0x97, 0x97),
        (0xA6, 0xAF),
    ]);
    let n105 = b.mk_concat(n103, n104);
    let n106 = b.mk_range_u8(0xB0, 0xB0);
    let n107 = b.mk_ranges_u8(&[
        (0x80, 0x8C),
        (0x8E, 0x90),
        (0x92, 0xA8),
        (0xAA, 0xB9),
        (0xBC, 0xBF),
    ]);
    let n108 = b.mk_concat(n106, n107);
    let n109 = b.mk_range_u8(0xB1, 0xB1);
    let n110 = b.mk_ranges_u8(&[
        (0x80, 0x84),
        (0x86, 0x88),
        (0x8A, 0x8D),
        (0x95, 0x96),
        (0x98, 0x9A),
        (0x9D, 0x9D),
        (0xA0, 0xA3),
        (0xA6, 0xAF),
    ]);
    let n111 = b.mk_concat(n109, n110);
    let n112 = b.mk_range_u8(0xB2, 0xB2);
    let n113 = b.mk_ranges_u8(&[
        (0x80, 0x83),
        (0x85, 0x8C),
        (0x8E, 0x90),
        (0x92, 0xA8),
        (0xAA, 0xB3),
        (0xB5, 0xB9),
        (0xBC, 0xBF),
    ]);
    let n114 = b.mk_concat(n112, n113);
    let n115 = b.mk_range_u8(0xB3, 0xB3);
    let n116 = b.mk_ranges_u8(&[
        (0x80, 0x84),
        (0x86, 0x88),
        (0x8A, 0x8D),
        (0x95, 0x96),
        (0x9D, 0x9E),
        (0xA0, 0xA3),
        (0xA6, 0xAF),
        (0xB1, 0xB3),
    ]);
    let n117 = b.mk_concat(n115, n116);
    let n118 = b.mk_range_u8(0xB4, 0xB4);
    let n119 = b.mk_ranges_u8(&[(0x80, 0x8C), (0x8E, 0x90), (0x92, 0xBF)]);
    let n120 = b.mk_concat(n118, n119);
    let n121 = b.mk_range_u8(0xB5, 0xB5);
    let n122 = b.mk_ranges_u8(&[
        (0x80, 0x84),
        (0x86, 0x88),
        (0x8A, 0x8E),
        (0x94, 0x97),
        (0x9F, 0xA3),
        (0xA6, 0xAF),
        (0xBA, 0xBF),
    ]);
    let n123 = b.mk_concat(n121, n122);
    let n124 = b.mk_range_u8(0xB6, 0xB6);
    let n125 = b.mk_ranges_u8(&[
        (0x81, 0x83),
        (0x85, 0x96),
        (0x9A, 0xB1),
        (0xB3, 0xBB),
        (0xBD, 0xBD),
    ]);
    let n126 = b.mk_concat(n124, n125);
    let n127 = b.mk_range_u8(0xB7, 0xB7);
    let n128 = b.mk_ranges_u8(&[
        (0x80, 0x86),
        (0x8A, 0x8A),
        (0x8F, 0x94),
        (0x96, 0x96),
        (0x98, 0x9F),
        (0xA6, 0xAF),
        (0xB2, 0xB3),
    ]);
    let n129 = b.mk_concat(n127, n128);
    let n130 = b.mk_range_u8(0xB8, 0xB8);
    let n131 = b.mk_range_u8(0x81, 0xBA);
    let n132 = b.mk_concat(n130, n131);
    let n133 = b.mk_range_u8(0xB9, 0xB9);
    let n134 = b.mk_ranges_u8(&[(0x80, 0x8E), (0x90, 0x99)]);
    let n135 = b.mk_concat(n133, n134);
    let n136 = b.mk_range_u8(0xBA, 0xBA);
    let n137 = b.mk_ranges_u8(&[
        (0x81, 0x82),
        (0x84, 0x84),
        (0x86, 0x8A),
        (0x8C, 0xA3),
        (0xA5, 0xA5),
        (0xA7, 0xBD),
    ]);
    let n138 = b.mk_concat(n136, n137);
    let n139 = b.mk_range_u8(0xBB, 0xBB);
    let n140 = b.mk_ranges_u8(&[
        (0x80, 0x84),
        (0x86, 0x86),
        (0x88, 0x8E),
        (0x90, 0x99),
        (0x9C, 0x9F),
    ]);
    let n141 = b.mk_concat(n139, n140);
    let n142 = b.mk_range_u8(0xBC, 0xBC);
    let n143 = b.mk_ranges_u8(&[
        (0x80, 0x80),
        (0x98, 0x99),
        (0xA0, 0xA9),
        (0xB5, 0xB5),
        (0xB7, 0xB7),
        (0xB9, 0xB9),
        (0xBE, 0xBF),
    ]);
    let n144 = b.mk_concat(n142, n143);
    let n145 = b.mk_range_u8(0xBD, 0xBD);
    let n146 = b.mk_ranges_u8(&[(0x80, 0x87), (0x89, 0xAC), (0xB1, 0xBF)]);
    let n147 = b.mk_concat(n145, n146);
    let n148 = b.mk_range_u8(0xBE, 0xBE);
    let n149 = b.mk_ranges_u8(&[(0x80, 0x84), (0x86, 0x97), (0x99, 0xBC)]);
    let n150 = b.mk_concat(n148, n149);
    let n151 = b.mk_range_u8(0xBF, 0xBF);
    let n152 = b.mk_range_u8(0x86, 0x86);
    let n153 = b.mk_concat(n151, n152);
    let n154 = b.mk_union(n150, n153);
    let n155 = b.mk_union(n147, n154);
    let n156 = b.mk_union(n144, n155);
    let n157 = b.mk_union(n141, n156);
    let n158 = b.mk_union(n138, n157);
    let n159 = b.mk_union(n135, n158);
    let n160 = b.mk_union(n132, n159);
    let n161 = b.mk_union(n129, n160);
    let n162 = b.mk_union(n126, n161);
    let n163 = b.mk_union(n123, n162);
    let n164 = b.mk_union(n120, n163);
    let n165 = b.mk_union(n117, n164);
    let n166 = b.mk_union(n114, n165);
    let n167 = b.mk_union(n111, n166);
    let n168 = b.mk_union(n108, n167);
    let n169 = b.mk_union(n105, n168);
    let n170 = b.mk_union(n102, n169);
    let n171 = b.mk_union(n99, n170);
    let n172 = b.mk_union(n96, n171);
    let n173 = b.mk_union(n93, n172);
    let n174 = b.mk_union(n90, n173);
    let n175 = b.mk_union(n87, n174);
    let n176 = b.mk_union(n84, n175);
    let n177 = b.mk_union(n81, n176);
    let n178 = b.mk_union(n78, n177);
    let n179 = b.mk_union(n75, n178);
    let n180 = b.mk_union(n72, n179);
    let n181 = b.mk_union(n70, n180);
    let n182 = b.mk_union(n67, n181);
    let n183 = b.mk_union(n64, n182);
    let n184 = b.mk_union(n61, n183);
    let n185 = b.mk_concat(n58, n184);
    let n186 = b.mk_range_u8(0xE1, 0xE1);
    let n187 = b.mk_ranges_u8(&[
        (0x80, 0x80),
        (0x84, 0x88),
        (0x91, 0x98),
        (0x9E, 0x9E),
        (0xAC, 0xAC),
        (0xAE, 0xAE),
        (0xB4, 0xBB),
    ]);
    let n188 = b.mk_concat(n187, n8);
    let n189 = b.mk_range_u8(0x81, 0x81);
    let n190 = b.mk_ranges_u8(&[(0x80, 0x89), (0x90, 0xBF)]);
    let n191 = b.mk_concat(n189, n190);
    let n192 = b.mk_range_u8(0x82, 0x82);
    let n193 = b.mk_ranges_u8(&[(0x80, 0x9D), (0xA0, 0xBF)]);
    let n194 = b.mk_concat(n192, n193);
    let n195 = b.mk_range_u8(0x83, 0x83);
    let n196 = b.mk_ranges_u8(&[
        (0x80, 0x85),
        (0x87, 0x87),
        (0x8D, 0x8D),
        (0x90, 0xBA),
        (0xBC, 0xBF),
    ]);
    let n197 = b.mk_concat(n195, n196);
    let n198 = b.mk_range_u8(0x89, 0x89);
    let n199 = b.mk_ranges_u8(&[
        (0x80, 0x88),
        (0x8A, 0x8D),
        (0x90, 0x96),
        (0x98, 0x98),
        (0x9A, 0x9D),
        (0xA0, 0xBF),
    ]);
    let n200 = b.mk_concat(n198, n199);
    let n201 = b.mk_range_u8(0x8A, 0x8A);
    let n202 = b.mk_ranges_u8(&[
        (0x80, 0x88),
        (0x8A, 0x8D),
        (0x90, 0xB0),
        (0xB2, 0xB5),
        (0xB8, 0xBE),
    ]);
    let n203 = b.mk_concat(n201, n202);
    let n204 = b.mk_range_u8(0x8B, 0x8B);
    let n205 = b.mk_ranges_u8(&[(0x80, 0x80), (0x82, 0x85), (0x88, 0x96), (0x98, 0xBF)]);
    let n206 = b.mk_concat(n204, n205);
    let n207 = b.mk_range_u8(0x8C, 0x8C);
    let n208 = b.mk_ranges_u8(&[(0x80, 0x90), (0x92, 0x95), (0x98, 0xBF)]);
    let n209 = b.mk_concat(n207, n208);
    let n210 = b.mk_range_u8(0x8D, 0x8D);
    let n211 = b.mk_ranges_u8(&[(0x80, 0x9A), (0x9D, 0x9F)]);
    let n212 = b.mk_concat(n210, n211);
    let n213 = b.mk_range_u8(0x8E, 0x8E);
    let n214 = b.mk_ranges_u8(&[(0x80, 0x8F), (0xA0, 0xBF)]);
    let n215 = b.mk_concat(n213, n214);
    let n216 = b.mk_range_u8(0x8F, 0x8F);
    let n217 = b.mk_ranges_u8(&[(0x80, 0xB5), (0xB8, 0xBD)]);
    let n218 = b.mk_concat(n216, n217);
    let n219 = b.mk_range_u8(0x90, 0x90);
    let n220 = b.mk_range_u8(0x81, 0xBF);
    let n221 = b.mk_concat(n219, n220);
    let n222 = b.mk_range_u8(0x99, 0x99);
    let n223 = b.mk_ranges_u8(&[(0x80, 0xAC), (0xAF, 0xBF)]);
    let n224 = b.mk_concat(n222, n223);
    let n225 = b.mk_range_u8(0x9A, 0x9A);
    let n226 = b.mk_ranges_u8(&[(0x81, 0x9A), (0xA0, 0xBF)]);
    let n227 = b.mk_concat(n225, n226);
    let n228 = b.mk_range_u8(0x9B, 0x9B);
    let n229 = b.mk_ranges_u8(&[(0x80, 0xAA), (0xAE, 0xB8)]);
    let n230 = b.mk_concat(n228, n229);
    let n231 = b.mk_range_u8(0x9C, 0x9C);
    let n232 = b.mk_ranges_u8(&[(0x80, 0x95), (0x9F, 0xB4)]);
    let n233 = b.mk_concat(n231, n232);
    let n234 = b.mk_range_u8(0x9D, 0x9D);
    let n235 = b.mk_ranges_u8(&[(0x80, 0x93), (0xA0, 0xAC), (0xAE, 0xB0), (0xB2, 0xB3)]);
    let n236 = b.mk_concat(n234, n235);
    let n237 = b.mk_range_u8(0x9F, 0x9F);
    let n238 = b.mk_ranges_u8(&[(0x80, 0x93), (0x97, 0x97), (0x9C, 0x9D), (0xA0, 0xA9)]);
    let n239 = b.mk_concat(n237, n238);
    let n240 = b.mk_ranges_u8(&[(0x8B, 0x8D), (0x8F, 0x99), (0xA0, 0xBF)]);
    let n241 = b.mk_concat(n59, n240);
    let n242 = b.mk_range_u8(0x80, 0xB8);
    let n243 = b.mk_concat(n62, n242);
    let n244 = b.mk_ranges_u8(&[(0x80, 0xAA), (0xB0, 0xBF)]);
    let n245 = b.mk_concat(n65, n244);
    let n246 = b.mk_range_u8(0x80, 0xB5);
    let n247 = b.mk_concat(n68, n246);
    let n248 = b.mk_ranges_u8(&[(0x80, 0x9E), (0xA0, 0xAB), (0xB0, 0xBB)]);
    let n249 = b.mk_concat(n71, n248);
    let n250 = b.mk_ranges_u8(&[(0x86, 0xAD), (0xB0, 0xB4)]);
    let n251 = b.mk_concat(n73, n250);
    let n252 = b.mk_ranges_u8(&[(0x80, 0xAB), (0xB0, 0xBF)]);
    let n253 = b.mk_concat(n76, n252);
    let n254 = b.mk_ranges_u8(&[(0x80, 0x89), (0x90, 0x99)]);
    let n255 = b.mk_concat(n79, n254);
    let n256 = b.mk_ranges_u8(&[(0x80, 0x9B), (0xA0, 0xBF)]);
    let n257 = b.mk_concat(n82, n256);
    let n258 = b.mk_ranges_u8(&[(0x80, 0x9E), (0xA0, 0xBC), (0xBF, 0xBF)]);
    let n259 = b.mk_concat(n85, n258);
    let n260 = b.mk_ranges_u8(&[(0x80, 0x89), (0x90, 0x99), (0xA7, 0xA7), (0xB0, 0xBF)]);
    let n261 = b.mk_concat(n88, n260);
    let n262 = b.mk_range_u8(0x80, 0x8E);
    let n263 = b.mk_concat(n91, n262);
    let n264 = b.mk_ranges_u8(&[(0x80, 0x8C), (0x90, 0x99), (0xAB, 0xB3)]);
    let n265 = b.mk_concat(n97, n264);
    let n266 = b.mk_range_u8(0x80, 0xB3);
    let n267 = b.mk_concat(n103, n266);
    let n268 = b.mk_range_u8(0x80, 0xB7);
    let n269 = b.mk_concat(n106, n268);
    let n270 = b.mk_ranges_u8(&[(0x80, 0x89), (0x8D, 0xBD)]);
    let n271 = b.mk_concat(n109, n270);
    let n272 = b.mk_ranges_u8(&[(0x80, 0x8A), (0x90, 0xBA), (0xBD, 0xBF)]);
    let n273 = b.mk_concat(n112, n272);
    let n274 = b.mk_ranges_u8(&[(0x90, 0x92), (0x94, 0xBA)]);
    let n275 = b.mk_concat(n115, n274);
    let n276 = b.mk_ranges_u8(&[(0x80, 0x95), (0x98, 0x9D), (0xA0, 0xBF)]);
    let n277 = b.mk_concat(n142, n276);
    let n278 = b.mk_ranges_u8(&[
        (0x80, 0x85),
        (0x88, 0x8D),
        (0x90, 0x97),
        (0x99, 0x99),
        (0x9B, 0x9B),
        (0x9D, 0x9D),
        (0x9F, 0xBD),
    ]);
    let n279 = b.mk_concat(n145, n278);
    let n280 = b.mk_ranges_u8(&[(0x80, 0xB4), (0xB6, 0xBC), (0xBE, 0xBE)]);
    let n281 = b.mk_concat(n148, n280);
    let n282 = b.mk_ranges_u8(&[
        (0x82, 0x84),
        (0x86, 0x8C),
        (0x90, 0x93),
        (0x96, 0x9B),
        (0xA0, 0xAC),
        (0xB2, 0xB4),
        (0xB6, 0xBC),
    ]);
    let n283 = b.mk_concat(n151, n282);
    let n284 = b.mk_union(n281, n283);
    let n285 = b.mk_union(n279, n284);
    let n286 = b.mk_union(n277, n285);
    let n287 = b.mk_union(n275, n286);
    let n288 = b.mk_union(n273, n287);
    let n289 = b.mk_union(n271, n288);
    let n290 = b.mk_union(n269, n289);
    let n291 = b.mk_union(n267, n290);
    let n292 = b.mk_union(n265, n291);
    let n293 = b.mk_union(n263, n292);
    let n294 = b.mk_union(n261, n293);
    let n295 = b.mk_union(n259, n294);
    let n296 = b.mk_union(n257, n295);
    let n297 = b.mk_union(n255, n296);
    let n298 = b.mk_union(n253, n297);
    let n299 = b.mk_union(n251, n298);
    let n300 = b.mk_union(n249, n299);
    let n301 = b.mk_union(n247, n300);
    let n302 = b.mk_union(n245, n301);
    let n303 = b.mk_union(n243, n302);
    let n304 = b.mk_union(n241, n303);
    let n305 = b.mk_union(n239, n304);
    let n306 = b.mk_union(n236, n305);
    let n307 = b.mk_union(n233, n306);
    let n308 = b.mk_union(n230, n307);
    let n309 = b.mk_union(n227, n308);
    let n310 = b.mk_union(n224, n309);
    let n311 = b.mk_union(n221, n310);
    let n312 = b.mk_union(n218, n311);
    let n313 = b.mk_union(n215, n312);
    let n314 = b.mk_union(n212, n313);
    let n315 = b.mk_union(n209, n314);
    let n316 = b.mk_union(n206, n315);
    let n317 = b.mk_union(n203, n316);
    let n318 = b.mk_union(n200, n317);
    let n319 = b.mk_union(n197, n318);
    let n320 = b.mk_union(n194, n319);
    let n321 = b.mk_union(n191, n320);
    let n322 = b.mk_union(n188, n321);
    let n323 = b.mk_concat(n186, n322);
    let n324 = b.mk_range_u8(0xE2, 0xE2);
    let n325 = b.mk_range_u8(0x80, 0x80);
    let n326 = b.mk_ranges_u8(&[(0x8C, 0x8D), (0xBF, 0xBF)]);
    let n327 = b.mk_concat(n325, n326);
    let n328 = b.mk_ranges_u8(&[(0x80, 0x80), (0x94, 0x94), (0xB1, 0xB1), (0xBF, 0xBF)]);
    let n329 = b.mk_concat(n189, n328);
    let n330 = b.mk_range_u8(0x90, 0x9C);
    let n331 = b.mk_concat(n192, n330);
    let n332 = b.mk_range_u8(0x90, 0xB0);
    let n333 = b.mk_concat(n195, n332);
    let n334 = b.mk_range_u8(0x84, 0x84);
    let n335 = b.mk_ranges_u8(&[
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
    let n336 = b.mk_concat(n334, n335);
    let n337 = b.mk_range_u8(0x85, 0x85);
    let n338 = b.mk_ranges_u8(&[(0x85, 0x89), (0x8E, 0x8E), (0xA0, 0xBF)]);
    let n339 = b.mk_concat(n337, n338);
    let n340 = b.mk_range_u8(0x80, 0x88);
    let n341 = b.mk_concat(n152, n340);
    let n342 = b.mk_range_u8(0x92, 0x92);
    let n343 = b.mk_range_u8(0xB6, 0xBF);
    let n344 = b.mk_concat(n342, n343);
    let n345 = b.mk_range_u8(0x93, 0x93);
    let n346 = b.mk_range_u8(0x80, 0xA9);
    let n347 = b.mk_concat(n345, n346);
    let n348 = b.mk_range_u8(0xB0, 0xB2);
    let n349 = b.mk_concat(n348, n8);
    let n350 = b.mk_ranges_u8(&[(0x80, 0xA4), (0xAB, 0xB3)]);
    let n351 = b.mk_concat(n115, n350);
    let n352 = b.mk_ranges_u8(&[(0x80, 0xA5), (0xA7, 0xA7), (0xAD, 0xAD), (0xB0, 0xBF)]);
    let n353 = b.mk_concat(n118, n352);
    let n354 = b.mk_ranges_u8(&[(0x80, 0xA7), (0xAF, 0xAF), (0xBF, 0xBF)]);
    let n355 = b.mk_concat(n121, n354);
    let n356 = b.mk_ranges_u8(&[
        (0x80, 0x96),
        (0xA0, 0xA6),
        (0xA8, 0xAE),
        (0xB0, 0xB6),
        (0xB8, 0xBE),
    ]);
    let n357 = b.mk_concat(n124, n356);
    let n358 = b.mk_ranges_u8(&[
        (0x80, 0x86),
        (0x88, 0x8E),
        (0x90, 0x96),
        (0x98, 0x9E),
        (0xA0, 0xBF),
    ]);
    let n359 = b.mk_concat(n127, n358);
    let n360 = b.mk_concat(n130, n103);
    let n361 = b.mk_union(n359, n360);
    let n362 = b.mk_union(n357, n361);
    let n363 = b.mk_union(n355, n362);
    let n364 = b.mk_union(n353, n363);
    let n365 = b.mk_union(n351, n364);
    let n366 = b.mk_union(n349, n365);
    let n367 = b.mk_union(n347, n366);
    let n368 = b.mk_union(n344, n367);
    let n369 = b.mk_union(n341, n368);
    let n370 = b.mk_union(n339, n369);
    let n371 = b.mk_union(n336, n370);
    let n372 = b.mk_union(n333, n371);
    let n373 = b.mk_union(n331, n372);
    let n374 = b.mk_union(n329, n373);
    let n375 = b.mk_union(n327, n374);
    let n376 = b.mk_concat(n324, n375);
    let n377 = b.mk_range_u8(0xE3, 0xE3);
    let n378 = b.mk_ranges_u8(&[(0x85, 0x87), (0xA1, 0xAF), (0xB1, 0xB5), (0xB8, 0xBC)]);
    let n379 = b.mk_concat(n325, n378);
    let n380 = b.mk_concat(n189, n220);
    let n381 = b.mk_ranges_u8(&[(0x80, 0x96), (0x99, 0x9A), (0x9D, 0x9F), (0xA1, 0xBF)]);
    let n382 = b.mk_concat(n192, n381);
    let n383 = b.mk_ranges_u8(&[(0x80, 0xBA), (0xBC, 0xBF)]);
    let n384 = b.mk_concat(n195, n383);
    let n385 = b.mk_ranges_u8(&[(0x85, 0xAF), (0xB1, 0xBF)]);
    let n386 = b.mk_concat(n334, n385);
    let n387 = b.mk_ranges_u8(&[(0x85, 0x85), (0x90, 0xBF)]);
    let n388 = b.mk_concat(n387, n8);
    let n389 = b.mk_ranges_u8(&[(0x80, 0x8E), (0xA0, 0xBF)]);
    let n390 = b.mk_concat(n152, n389);
    let n391 = b.mk_range_u8(0x87, 0x87);
    let n392 = b.mk_range_u8(0xB0, 0xBF);
    let n393 = b.mk_concat(n391, n392);
    let n394 = b.mk_union(n390, n393);
    let n395 = b.mk_union(n388, n394);
    let n396 = b.mk_union(n386, n395);
    let n397 = b.mk_union(n384, n396);
    let n398 = b.mk_union(n382, n397);
    let n399 = b.mk_union(n380, n398);
    let n400 = b.mk_union(n379, n399);
    let n401 = b.mk_concat(n377, n400);
    let n402 = b.mk_range_u8(0xE4, 0xE4);
    let n403 = b.mk_ranges_u8(&[(0x80, 0xB6), (0xB8, 0xBF)]);
    let n404 = b.mk_concat(n403, n8);
    let n405 = b.mk_concat(n402, n404);
    let n406 = b.mk_ranges_u8(&[(0xE5, 0xE9), (0xEB, 0xEC)]);
    let n407 = b.mk_concat(n8, n8);
    let n408 = b.mk_concat(n406, n407);
    let n409 = b.mk_range_u8(0xEA, 0xEA);
    let n410 = b.mk_ranges_u8(&[
        (0x80, 0x91),
        (0x94, 0x97),
        (0x9A, 0x9A),
        (0x9D, 0x9D),
        (0xA2, 0xA2),
        (0xA6, 0xA6),
        (0xAA, 0xAA),
        (0xAE, 0xAE),
        (0xB0, 0xBF),
    ]);
    let n411 = b.mk_concat(n410, n8);
    let n412 = b.mk_range_u8(0x80, 0x8C);
    let n413 = b.mk_concat(n342, n412);
    let n414 = b.mk_range_u8(0x90, 0xBD);
    let n415 = b.mk_concat(n345, n414);
    let n416 = b.mk_range_u8(0x98, 0x98);
    let n417 = b.mk_ranges_u8(&[(0x80, 0x8C), (0x90, 0xAB)]);
    let n418 = b.mk_concat(n416, n417);
    let n419 = b.mk_ranges_u8(&[(0x80, 0xB2), (0xB4, 0xBD), (0xBF, 0xBF)]);
    let n420 = b.mk_concat(n222, n419);
    let n421 = b.mk_concat(n228, n53);
    let n422 = b.mk_ranges_u8(&[(0x97, 0x9F), (0xA2, 0xBF)]);
    let n423 = b.mk_concat(n231, n422);
    let n424 = b.mk_range_u8(0x9E, 0x9E);
    let n425 = b.mk_ranges_u8(&[(0x80, 0x88), (0x8B, 0xBF)]);
    let n426 = b.mk_concat(n424, n425);
    let n427 = b.mk_ranges_u8(&[
        (0x80, 0x8D),
        (0x90, 0x91),
        (0x93, 0x93),
        (0x95, 0x9C),
        (0xB2, 0xBF),
    ]);
    let n428 = b.mk_concat(n237, n427);
    let n429 = b.mk_ranges_u8(&[(0x80, 0xA7), (0xAC, 0xAC)]);
    let n430 = b.mk_concat(n59, n429);
    let n431 = b.mk_concat(n62, n266);
    let n432 = b.mk_ranges_u8(&[
        (0x80, 0x85),
        (0x90, 0x99),
        (0xA0, 0xB7),
        (0xBB, 0xBB),
        (0xBD, 0xBF),
    ]);
    let n433 = b.mk_concat(n68, n432);
    let n434 = b.mk_ranges_u8(&[(0x80, 0xAD), (0xB0, 0xBF)]);
    let n435 = b.mk_concat(n71, n434);
    let n436 = b.mk_ranges_u8(&[(0x80, 0x93), (0xA0, 0xBC)]);
    let n437 = b.mk_concat(n73, n436);
    let n438 = b.mk_ranges_u8(&[(0x80, 0x80), (0x8F, 0x99), (0xA0, 0xBE)]);
    let n439 = b.mk_concat(n79, n438);
    let n440 = b.mk_range_u8(0x80, 0xB6);
    let n441 = b.mk_concat(n82, n440);
    let n442 = b.mk_ranges_u8(&[(0x80, 0x8D), (0x90, 0x99), (0xA0, 0xB6), (0xBA, 0xBF)]);
    let n443 = b.mk_concat(n85, n442);
    let n444 = b.mk_ranges_u8(&[(0x80, 0x82), (0x9B, 0x9D), (0xA0, 0xAF), (0xB2, 0xB6)]);
    let n445 = b.mk_concat(n91, n444);
    let n446 = b.mk_ranges_u8(&[
        (0x81, 0x86),
        (0x89, 0x8E),
        (0x91, 0x96),
        (0xA0, 0xA6),
        (0xA8, 0xAE),
        (0xB0, 0xBF),
    ]);
    let n447 = b.mk_concat(n94, n446);
    let n448 = b.mk_ranges_u8(&[(0x80, 0x9A), (0x9C, 0xA9), (0xB0, 0xBF)]);
    let n449 = b.mk_concat(n97, n448);
    let n450 = b.mk_ranges_u8(&[(0x80, 0xAA), (0xAC, 0xAD), (0xB0, 0xB9)]);
    let n451 = b.mk_concat(n103, n450);
    let n452 = b.mk_union(n449, n451);
    let n453 = b.mk_union(n447, n452);
    let n454 = b.mk_union(n445, n453);
    let n455 = b.mk_union(n443, n454);
    let n456 = b.mk_union(n441, n455);
    let n457 = b.mk_union(n439, n456);
    let n458 = b.mk_union(n437, n457);
    let n459 = b.mk_union(n435, n458);
    let n460 = b.mk_union(n433, n459);
    let n461 = b.mk_union(n431, n460);
    let n462 = b.mk_union(n430, n461);
    let n463 = b.mk_union(n428, n462);
    let n464 = b.mk_union(n426, n463);
    let n465 = b.mk_union(n423, n464);
    let n466 = b.mk_union(n421, n465);
    let n467 = b.mk_union(n420, n466);
    let n468 = b.mk_union(n418, n467);
    let n469 = b.mk_union(n415, n468);
    let n470 = b.mk_union(n413, n469);
    let n471 = b.mk_union(n411, n470);
    let n472 = b.mk_concat(n409, n471);
    let n473 = b.mk_range_u8(0xED, 0xED);
    let n474 = b.mk_range_u8(0x80, 0x9D);
    let n475 = b.mk_concat(n474, n8);
    let n476 = b.mk_ranges_u8(&[(0x80, 0xA3), (0xB0, 0xBF)]);
    let n477 = b.mk_concat(n424, n476);
    let n478 = b.mk_ranges_u8(&[(0x80, 0x86), (0x8B, 0xBB)]);
    let n479 = b.mk_concat(n237, n478);
    let n480 = b.mk_union(n477, n479);
    let n481 = b.mk_union(n475, n480);
    let n482 = b.mk_concat(n473, n481);
    let n483 = b.mk_range_u8(0xEF, 0xEF);
    let n484 = b.mk_ranges_u8(&[(0xA4, 0xA8), (0xAA, 0xAA), (0xB0, 0xB3), (0xBA, 0xBA)]);
    let n485 = b.mk_concat(n484, n8);
    let n486 = b.mk_concat(n85, n434);
    let n487 = b.mk_range_u8(0x80, 0x99);
    let n488 = b.mk_concat(n91, n487);
    let n489 = b.mk_ranges_u8(&[
        (0x80, 0x86),
        (0x93, 0x97),
        (0x9D, 0xA8),
        (0xAA, 0xB6),
        (0xB8, 0xBC),
        (0xBE, 0xBE),
    ]);
    let n490 = b.mk_concat(n94, n489);
    let n491 = b.mk_ranges_u8(&[(0x80, 0x81), (0x83, 0x84), (0x86, 0xBF)]);
    let n492 = b.mk_concat(n97, n491);
    let n493 = b.mk_concat(n100, n53);
    let n494 = b.mk_range_u8(0x93, 0xBF);
    let n495 = b.mk_concat(n103, n494);
    let n496 = b.mk_range_u8(0x80, 0xBD);
    let n497 = b.mk_concat(n118, n496);
    let n498 = b.mk_concat(n121, n47);
    let n499 = b.mk_ranges_u8(&[(0x80, 0x8F), (0x92, 0xBF)]);
    let n500 = b.mk_concat(n124, n499);
    let n501 = b.mk_ranges_u8(&[(0x80, 0x87), (0xB0, 0xBB)]);
    let n502 = b.mk_concat(n127, n501);
    let n503 = b.mk_ranges_u8(&[(0x80, 0x8F), (0xA0, 0xAF), (0xB3, 0xB4)]);
    let n504 = b.mk_concat(n130, n503);
    let n505 = b.mk_ranges_u8(&[(0x8D, 0x8F), (0xB0, 0xB4), (0xB6, 0xBF)]);
    let n506 = b.mk_concat(n133, n505);
    let n507 = b.mk_range_u8(0x80, 0xBC);
    let n508 = b.mk_concat(n139, n507);
    let n509 = b.mk_ranges_u8(&[(0x90, 0x99), (0xA1, 0xBA), (0xBF, 0xBF)]);
    let n510 = b.mk_concat(n142, n509);
    let n511 = b.mk_ranges_u8(&[(0x81, 0x9A), (0xA6, 0xBF)]);
    let n512 = b.mk_concat(n145, n511);
    let n513 = b.mk_range_u8(0x80, 0xBE);
    let n514 = b.mk_concat(n148, n513);
    let n515 = b.mk_ranges_u8(&[(0x82, 0x87), (0x8A, 0x8F), (0x92, 0x97), (0x9A, 0x9C)]);
    let n516 = b.mk_concat(n151, n515);
    let n517 = b.mk_union(n514, n516);
    let n518 = b.mk_union(n512, n517);
    let n519 = b.mk_union(n510, n518);
    let n520 = b.mk_union(n508, n519);
    let n521 = b.mk_union(n506, n520);
    let n522 = b.mk_union(n504, n521);
    let n523 = b.mk_union(n502, n522);
    let n524 = b.mk_union(n500, n523);
    let n525 = b.mk_union(n498, n524);
    let n526 = b.mk_union(n497, n525);
    let n527 = b.mk_union(n495, n526);
    let n528 = b.mk_union(n493, n527);
    let n529 = b.mk_union(n492, n528);
    let n530 = b.mk_union(n490, n529);
    let n531 = b.mk_union(n488, n530);
    let n532 = b.mk_union(n486, n531);
    let n533 = b.mk_union(n485, n532);
    let n534 = b.mk_concat(n483, n533);
    let n535 = b.mk_range_u8(0xF0, 0xF0);
    let n536 = b.mk_ranges_u8(&[
        (0x80, 0x8B),
        (0x8D, 0xA6),
        (0xA8, 0xBA),
        (0xBC, 0xBD),
        (0xBF, 0xBF),
    ]);
    let n537 = b.mk_concat(n325, n536);
    let n538 = b.mk_ranges_u8(&[(0x80, 0x8D), (0x90, 0x9D)]);
    let n539 = b.mk_concat(n189, n538);
    let n540 = b.mk_ranges_u8(&[(0x82, 0x82), (0x90, 0x91), (0x98, 0x9B), (0xB0, 0xB0)]);
    let n541 = b.mk_concat(n540, n8);
    let n542 = b.mk_range_u8(0x80, 0xBA);
    let n543 = b.mk_concat(n195, n542);
    let n544 = b.mk_range_u8(0x80, 0xB4);
    let n545 = b.mk_concat(n337, n544);
    let n546 = b.mk_concat(n391, n145);
    let n547 = b.mk_ranges_u8(&[(0x80, 0x9C), (0xA0, 0xBF)]);
    let n548 = b.mk_concat(n201, n547);
    let n549 = b.mk_ranges_u8(&[(0x80, 0x90), (0xA0, 0xA0)]);
    let n550 = b.mk_concat(n204, n549);
    let n551 = b.mk_ranges_u8(&[(0x80, 0x9F), (0xAD, 0xBF)]);
    let n552 = b.mk_concat(n207, n551);
    let n553 = b.mk_ranges_u8(&[(0x80, 0x8A), (0x90, 0xBA)]);
    let n554 = b.mk_concat(n210, n553);
    let n555 = b.mk_concat(n213, n193);
    let n556 = b.mk_ranges_u8(&[(0x80, 0x83), (0x88, 0x8F), (0x91, 0x95)]);
    let n557 = b.mk_concat(n216, n556);
    let n558 = b.mk_ranges_u8(&[(0x80, 0x9D), (0xA0, 0xA9), (0xB0, 0xBF)]);
    let n559 = b.mk_concat(n342, n558);
    let n560 = b.mk_ranges_u8(&[(0x80, 0x93), (0x98, 0xBB)]);
    let n561 = b.mk_concat(n345, n560);
    let n562 = b.mk_range_u8(0x94, 0x94);
    let n563 = b.mk_ranges_u8(&[(0x80, 0xA7), (0xB0, 0xBF)]);
    let n564 = b.mk_concat(n562, n563);
    let n565 = b.mk_range_u8(0x95, 0x95);
    let n566 = b.mk_ranges_u8(&[(0x80, 0xA3), (0xB0, 0xBA), (0xBC, 0xBF)]);
    let n567 = b.mk_concat(n565, n566);
    let n568 = b.mk_range_u8(0x96, 0x96);
    let n569 = b.mk_ranges_u8(&[
        (0x80, 0x8A),
        (0x8C, 0x92),
        (0x94, 0x95),
        (0x97, 0xA1),
        (0xA3, 0xB1),
        (0xB3, 0xB9),
        (0xBB, 0xBC),
    ]);
    let n570 = b.mk_concat(n568, n569);
    let n571 = b.mk_range_u8(0x97, 0x97);
    let n572 = b.mk_concat(n571, n266);
    let n573 = b.mk_concat(n231, n440);
    let n574 = b.mk_ranges_u8(&[(0x80, 0x95), (0xA0, 0xA7)]);
    let n575 = b.mk_concat(n234, n574);
    let n576 = b.mk_ranges_u8(&[(0x80, 0x85), (0x87, 0xB0), (0xB2, 0xBA)]);
    let n577 = b.mk_concat(n424, n576);
    let n578 = b.mk_ranges_u8(&[
        (0x80, 0x85),
        (0x88, 0x88),
        (0x8A, 0xB5),
        (0xB7, 0xB8),
        (0xBC, 0xBC),
        (0xBF, 0xBF),
    ]);
    let n579 = b.mk_concat(n59, n578);
    let n580 = b.mk_ranges_u8(&[(0x80, 0x95), (0xA0, 0xB6)]);
    let n581 = b.mk_concat(n62, n580);
    let n582 = b.mk_range_u8(0x80, 0x9E);
    let n583 = b.mk_concat(n65, n582);
    let n584 = b.mk_ranges_u8(&[(0xA0, 0xB2), (0xB4, 0xB5)]);
    let n585 = b.mk_concat(n68, n584);
    let n586 = b.mk_ranges_u8(&[(0x80, 0x95), (0xA0, 0xB9)]);
    let n587 = b.mk_concat(n71, n586);
    let n588 = b.mk_ranges_u8(&[(0x80, 0xB7), (0xBE, 0xBF)]);
    let n589 = b.mk_concat(n76, n588);
    let n590 = b.mk_ranges_u8(&[
        (0x80, 0x83),
        (0x85, 0x86),
        (0x8C, 0x93),
        (0x95, 0x97),
        (0x99, 0xB5),
        (0xB8, 0xBA),
        (0xBF, 0xBF),
    ]);
    let n591 = b.mk_concat(n82, n590);
    let n592 = b.mk_range_u8(0xA0, 0xBC);
    let n593 = b.mk_concat(n85, n592);
    let n594 = b.mk_range_u8(0x80, 0x9C);
    let n595 = b.mk_concat(n88, n594);
    let n596 = b.mk_ranges_u8(&[(0x80, 0x87), (0x89, 0xA6)]);
    let n597 = b.mk_concat(n91, n596);
    let n598 = b.mk_concat(n94, n246);
    let n599 = b.mk_ranges_u8(&[(0x80, 0x95), (0xA0, 0xB2)]);
    let n600 = b.mk_concat(n97, n599);
    let n601 = b.mk_range_u8(0x80, 0x91);
    let n602 = b.mk_concat(n100, n601);
    let n603 = b.mk_concat(n109, n340);
    let n604 = b.mk_range_u8(0xB2, 0xB3);
    let n605 = b.mk_range_u8(0x80, 0xB2);
    let n606 = b.mk_concat(n604, n605);
    let n607 = b.mk_ranges_u8(&[(0x80, 0xA7), (0xB0, 0xB9)]);
    let n608 = b.mk_concat(n118, n607);
    let n609 = b.mk_ranges_u8(&[(0x80, 0xA5), (0xA9, 0xAD), (0xAF, 0xBF)]);
    let n610 = b.mk_concat(n121, n609);
    let n611 = b.mk_range_u8(0x80, 0x85);
    let n612 = b.mk_concat(n124, n611);
    let n613 = b.mk_ranges_u8(&[(0x80, 0xA9), (0xAB, 0xAC), (0xB0, 0xB1)]);
    let n614 = b.mk_concat(n136, n613);
    let n615 = b.mk_ranges_u8(&[(0x82, 0x84), (0xBC, 0xBF)]);
    let n616 = b.mk_concat(n139, n615);
    let n617 = b.mk_ranges_u8(&[(0x80, 0x9C), (0xA7, 0xA7), (0xB0, 0xBF)]);
    let n618 = b.mk_concat(n142, n617);
    let n619 = b.mk_ranges_u8(&[(0x80, 0x90), (0xB0, 0xBF)]);
    let n620 = b.mk_concat(n145, n619);
    let n621 = b.mk_ranges_u8(&[(0x80, 0x85), (0xB0, 0xBF)]);
    let n622 = b.mk_concat(n148, n621);
    let n623 = b.mk_ranges_u8(&[(0x80, 0x84), (0xA0, 0xB6)]);
    let n624 = b.mk_concat(n151, n623);
    let n625 = b.mk_union(n622, n624);
    let n626 = b.mk_union(n620, n625);
    let n627 = b.mk_union(n618, n626);
    let n628 = b.mk_union(n616, n627);
    let n629 = b.mk_union(n614, n628);
    let n630 = b.mk_union(n612, n629);
    let n631 = b.mk_union(n610, n630);
    let n632 = b.mk_union(n608, n631);
    let n633 = b.mk_union(n606, n632);
    let n634 = b.mk_union(n603, n633);
    let n635 = b.mk_union(n602, n634);
    let n636 = b.mk_union(n600, n635);
    let n637 = b.mk_union(n598, n636);
    let n638 = b.mk_union(n597, n637);
    let n639 = b.mk_union(n595, n638);
    let n640 = b.mk_union(n593, n639);
    let n641 = b.mk_union(n591, n640);
    let n642 = b.mk_union(n589, n641);
    let n643 = b.mk_union(n587, n642);
    let n644 = b.mk_union(n585, n643);
    let n645 = b.mk_union(n583, n644);
    let n646 = b.mk_union(n581, n645);
    let n647 = b.mk_union(n579, n646);
    let n648 = b.mk_union(n577, n647);
    let n649 = b.mk_union(n575, n648);
    let n650 = b.mk_union(n573, n649);
    let n651 = b.mk_union(n572, n650);
    let n652 = b.mk_union(n570, n651);
    let n653 = b.mk_union(n567, n652);
    let n654 = b.mk_union(n564, n653);
    let n655 = b.mk_union(n561, n654);
    let n656 = b.mk_union(n559, n655);
    let n657 = b.mk_union(n557, n656);
    let n658 = b.mk_union(n555, n657);
    let n659 = b.mk_union(n554, n658);
    let n660 = b.mk_union(n552, n659);
    let n661 = b.mk_union(n550, n660);
    let n662 = b.mk_union(n548, n661);
    let n663 = b.mk_union(n546, n662);
    let n664 = b.mk_union(n545, n663);
    let n665 = b.mk_union(n543, n664);
    let n666 = b.mk_union(n541, n665);
    let n667 = b.mk_union(n539, n666);
    let n668 = b.mk_union(n537, n667);
    let n669 = b.mk_concat(n219, n668);
    let n670 = b.mk_range_u8(0x91, 0x91);
    let n671 = b.mk_ranges_u8(&[
        (0x80, 0x80),
        (0x86, 0x86),
        (0x90, 0x90),
        (0x92, 0x92),
        (0x98, 0x98),
    ]);
    let n672 = b.mk_concat(n671, n8);
    let n673 = b.mk_ranges_u8(&[(0x80, 0x86), (0xA6, 0xB5), (0xBF, 0xBF)]);
    let n674 = b.mk_concat(n189, n673);
    let n675 = b.mk_ranges_u8(&[(0x82, 0x82), (0xA0, 0xA0)]);
    let n676 = b.mk_concat(n675, n542);
    let n677 = b.mk_ranges_u8(&[(0x82, 0x82), (0x90, 0xA8), (0xB0, 0xB9)]);
    let n678 = b.mk_concat(n195, n677);
    let n679 = b.mk_ranges_u8(&[(0x80, 0xB4), (0xB6, 0xBF)]);
    let n680 = b.mk_concat(n334, n679);
    let n681 = b.mk_ranges_u8(&[(0x84, 0x87), (0x90, 0xB3), (0xB6, 0xB6)]);
    let n682 = b.mk_concat(n337, n681);
    let n683 = b.mk_ranges_u8(&[(0x80, 0x84), (0x89, 0x8C), (0x8E, 0x9A), (0x9C, 0x9C)]);
    let n684 = b.mk_concat(n391, n683);
    let n685 = b.mk_range_u8(0x88, 0x88);
    let n686 = b.mk_ranges_u8(&[(0x80, 0x91), (0x93, 0xB7), (0xBE, 0xBF)]);
    let n687 = b.mk_concat(n685, n686);
    let n688 = b.mk_range_u8(0x80, 0x81);
    let n689 = b.mk_concat(n198, n688);
    let n690 = b.mk_ranges_u8(&[
        (0x80, 0x86),
        (0x88, 0x88),
        (0x8A, 0x8D),
        (0x8F, 0x9D),
        (0x9F, 0xA8),
        (0xB0, 0xBF),
    ]);
    let n691 = b.mk_concat(n201, n690);
    let n692 = b.mk_ranges_u8(&[(0x80, 0xAA), (0xB0, 0xB9)]);
    let n693 = b.mk_concat(n204, n692);
    let n694 = b.mk_ranges_u8(&[
        (0x80, 0x83),
        (0x85, 0x8C),
        (0x8F, 0x90),
        (0x93, 0xA8),
        (0xAA, 0xB0),
        (0xB2, 0xB3),
        (0xB5, 0xB9),
        (0xBB, 0xBF),
    ]);
    let n695 = b.mk_concat(n207, n694);
    let n696 = b.mk_ranges_u8(&[
        (0x80, 0x84),
        (0x87, 0x88),
        (0x8B, 0x8D),
        (0x90, 0x90),
        (0x97, 0x97),
        (0x9D, 0xA3),
        (0xA6, 0xAC),
        (0xB0, 0xB4),
    ]);
    let n697 = b.mk_concat(n210, n696);
    let n698 = b.mk_ranges_u8(&[
        (0x80, 0x89),
        (0x8B, 0x8B),
        (0x8E, 0x8E),
        (0x90, 0xB5),
        (0xB7, 0xBF),
    ]);
    let n699 = b.mk_concat(n213, n698);
    let n700 = b.mk_ranges_u8(&[
        (0x80, 0x80),
        (0x82, 0x82),
        (0x85, 0x85),
        (0x87, 0x8A),
        (0x8C, 0x93),
        (0xA1, 0xA2),
    ]);
    let n701 = b.mk_concat(n216, n700);
    let n702 = b.mk_ranges_u8(&[(0x80, 0x8A), (0x90, 0x99), (0x9E, 0xA1)]);
    let n703 = b.mk_concat(n670, n702);
    let n704 = b.mk_ranges_u8(&[(0x80, 0x85), (0x87, 0x87), (0x90, 0x99)]);
    let n705 = b.mk_concat(n345, n704);
    let n706 = b.mk_ranges_u8(&[(0x80, 0xB5), (0xB8, 0xBF)]);
    let n707 = b.mk_concat(n568, n706);
    let n708 = b.mk_ranges_u8(&[(0x80, 0x80), (0x98, 0x9D)]);
    let n709 = b.mk_concat(n571, n708);
    let n710 = b.mk_ranges_u8(&[(0x80, 0x80), (0x84, 0x84), (0x90, 0x99)]);
    let n711 = b.mk_concat(n222, n710);
    let n712 = b.mk_ranges_u8(&[(0x9A, 0x9A), (0xAB, 0xAB)]);
    let n713 = b.mk_concat(n712, n242);
    let n714 = b.mk_ranges_u8(&[(0x80, 0x89), (0x90, 0xA3)]);
    let n715 = b.mk_concat(n228, n714);
    let n716 = b.mk_ranges_u8(&[(0x80, 0x9A), (0x9D, 0xAB), (0xB0, 0xB9)]);
    let n717 = b.mk_concat(n231, n716);
    let n718 = b.mk_range_u8(0x80, 0x86);
    let n719 = b.mk_concat(n234, n718);
    let n720 = b.mk_range_u8(0xA0, 0xBF);
    let n721 = b.mk_concat(n65, n720);
    let n722 = b.mk_ranges_u8(&[(0x80, 0xA9), (0xBF, 0xBF)]);
    let n723 = b.mk_concat(n68, n722);
    let n724 = b.mk_ranges_u8(&[
        (0x80, 0x86),
        (0x89, 0x89),
        (0x8C, 0x93),
        (0x95, 0x96),
        (0x98, 0xB5),
        (0xB7, 0xB8),
        (0xBB, 0xBF),
    ]);
    let n725 = b.mk_concat(n71, n724);
    let n726 = b.mk_ranges_u8(&[(0x80, 0x83), (0x90, 0x99)]);
    let n727 = b.mk_concat(n73, n726);
    let n728 = b.mk_ranges_u8(&[(0xA0, 0xA7), (0xAA, 0xBF)]);
    let n729 = b.mk_concat(n76, n728);
    let n730 = b.mk_ranges_u8(&[(0x80, 0x97), (0x9A, 0xA1), (0xA3, 0xA4)]);
    let n731 = b.mk_concat(n79, n730);
    let n732 = b.mk_concat(n82, n513);
    let n733 = b.mk_ranges_u8(&[(0x87, 0x87), (0x90, 0xBF)]);
    let n734 = b.mk_concat(n85, n733);
    let n735 = b.mk_ranges_u8(&[(0x80, 0x99), (0x9D, 0x9D), (0xB0, 0xBF)]);
    let n736 = b.mk_concat(n88, n735);
    let n737 = b.mk_ranges_u8(&[(0x80, 0xA0), (0xB0, 0xB9)]);
    let n738 = b.mk_concat(n103, n737);
    let n739 = b.mk_ranges_u8(&[(0x80, 0x88), (0x8A, 0xB6), (0xB8, 0xBF)]);
    let n740 = b.mk_concat(n106, n739);
    let n741 = b.mk_ranges_u8(&[(0x80, 0x80), (0x90, 0x99), (0xB2, 0xBF)]);
    let n742 = b.mk_concat(n109, n741);
    let n743 = b.mk_ranges_u8(&[(0x80, 0x8F), (0x92, 0xA7), (0xA9, 0xB6)]);
    let n744 = b.mk_concat(n112, n743);
    let n745 = b.mk_ranges_u8(&[
        (0x80, 0x86),
        (0x88, 0x89),
        (0x8B, 0xB6),
        (0xBA, 0xBA),
        (0xBC, 0xBD),
        (0xBF, 0xBF),
    ]);
    let n746 = b.mk_concat(n118, n745);
    let n747 = b.mk_ranges_u8(&[
        (0x80, 0x87),
        (0x90, 0x99),
        (0xA0, 0xA5),
        (0xA7, 0xA8),
        (0xAA, 0xBF),
    ]);
    let n748 = b.mk_concat(n121, n747);
    let n749 = b.mk_ranges_u8(&[(0x80, 0x8E), (0x90, 0x91), (0x93, 0x98), (0xA0, 0xA9)]);
    let n750 = b.mk_concat(n124, n749);
    let n751 = b.mk_range_u8(0xA0, 0xB6);
    let n752 = b.mk_concat(n139, n751);
    let n753 = b.mk_ranges_u8(&[(0x80, 0x90), (0x92, 0xBA), (0xBE, 0xBF)]);
    let n754 = b.mk_concat(n142, n753);
    let n755 = b.mk_ranges_u8(&[(0x80, 0x82), (0x90, 0x9A)]);
    let n756 = b.mk_concat(n145, n755);
    let n757 = b.mk_concat(n148, n106);
    let n758 = b.mk_union(n756, n757);
    let n759 = b.mk_union(n754, n758);
    let n760 = b.mk_union(n752, n759);
    let n761 = b.mk_union(n750, n760);
    let n762 = b.mk_union(n748, n761);
    let n763 = b.mk_union(n746, n762);
    let n764 = b.mk_union(n744, n763);
    let n765 = b.mk_union(n742, n764);
    let n766 = b.mk_union(n740, n765);
    let n767 = b.mk_union(n738, n766);
    let n768 = b.mk_union(n736, n767);
    let n769 = b.mk_union(n734, n768);
    let n770 = b.mk_union(n732, n769);
    let n771 = b.mk_union(n731, n770);
    let n772 = b.mk_union(n729, n771);
    let n773 = b.mk_union(n727, n772);
    let n774 = b.mk_union(n725, n773);
    let n775 = b.mk_union(n723, n774);
    let n776 = b.mk_union(n721, n775);
    let n777 = b.mk_union(n719, n776);
    let n778 = b.mk_union(n717, n777);
    let n779 = b.mk_union(n715, n778);
    let n780 = b.mk_union(n713, n779);
    let n781 = b.mk_union(n711, n780);
    let n782 = b.mk_union(n709, n781);
    let n783 = b.mk_union(n707, n782);
    let n784 = b.mk_union(n705, n783);
    let n785 = b.mk_union(n703, n784);
    let n786 = b.mk_union(n701, n785);
    let n787 = b.mk_union(n699, n786);
    let n788 = b.mk_union(n697, n787);
    let n789 = b.mk_union(n695, n788);
    let n790 = b.mk_union(n693, n789);
    let n791 = b.mk_union(n691, n790);
    let n792 = b.mk_union(n689, n791);
    let n793 = b.mk_union(n687, n792);
    let n794 = b.mk_union(n684, n793);
    let n795 = b.mk_union(n682, n794);
    let n796 = b.mk_union(n680, n795);
    let n797 = b.mk_union(n678, n796);
    let n798 = b.mk_union(n676, n797);
    let n799 = b.mk_union(n674, n798);
    let n800 = b.mk_union(n672, n799);
    let n801 = b.mk_concat(n670, n800);
    let n802 = b.mk_ranges_u8(&[(0x80, 0x8D), (0x90, 0x90), (0x92, 0x94)]);
    let n803 = b.mk_concat(n802, n8);
    let n804 = b.mk_concat(n213, n487);
    let n805 = b.mk_range_u8(0x80, 0xAE);
    let n806 = b.mk_concat(n670, n805);
    let n807 = b.mk_range_u8(0x80, 0x83);
    let n808 = b.mk_concat(n565, n807);
    let n809 = b.mk_concat(n148, n47);
    let n810 = b.mk_range_u8(0x80, 0xB0);
    let n811 = b.mk_concat(n151, n810);
    let n812 = b.mk_union(n809, n811);
    let n813 = b.mk_union(n808, n812);
    let n814 = b.mk_union(n806, n813);
    let n815 = b.mk_union(n804, n814);
    let n816 = b.mk_union(n803, n815);
    let n817 = b.mk_concat(n342, n816);
    let n818 = b.mk_concat(n499, n8);
    let n819 = b.mk_range_u8(0x80, 0xAF);
    let n820 = b.mk_concat(n219, n819);
    let n821 = b.mk_ranges_u8(&[(0x80, 0x95), (0xA0, 0xBF)]);
    let n822 = b.mk_concat(n670, n821);
    let n823 = b.mk_union(n820, n822);
    let n824 = b.mk_union(n818, n823);
    let n825 = b.mk_concat(n345, n824);
    let n826 = b.mk_ranges_u8(&[(0x80, 0x8E), (0x90, 0x98)]);
    let n827 = b.mk_concat(n826, n8);
    let n828 = b.mk_concat(n216, n542);
    let n829 = b.mk_concat(n222, n718);
    let n830 = b.mk_union(n828, n829);
    let n831 = b.mk_union(n827, n830);
    let n832 = b.mk_concat(n562, n831);
    let n833 = b.mk_range_u8(0x80, 0xB9);
    let n834 = b.mk_concat(n334, n833);
    let n835 = b.mk_ranges_u8(&[(0xA0, 0xA7), (0xB9, 0xB9), (0xBC, 0xBC)]);
    let n836 = b.mk_concat(n835, n8);
    let n837 = b.mk_concat(n82, n242);
    let n838 = b.mk_ranges_u8(&[(0x80, 0x9E), (0xA0, 0xA9), (0xB0, 0xBF)]);
    let n839 = b.mk_concat(n85, n838);
    let n840 = b.mk_concat(n88, n513);
    let n841 = b.mk_ranges_u8(&[(0x80, 0x89), (0x90, 0xAD), (0xB0, 0xB4)]);
    let n842 = b.mk_concat(n91, n841);
    let n843 = b.mk_concat(n94, n440);
    let n844 = b.mk_ranges_u8(&[(0x80, 0x83), (0x90, 0x99), (0xA3, 0xB7), (0xBD, 0xBF)]);
    let n845 = b.mk_concat(n97, n844);
    let n846 = b.mk_range_u8(0x80, 0x8F);
    let n847 = b.mk_concat(n100, n846);
    let n848 = b.mk_ranges_u8(&[(0x80, 0xAC), (0xB0, 0xB9)]);
    let n849 = b.mk_concat(n121, n848);
    let n850 = b.mk_ranges_u8(&[(0x80, 0x8A), (0x8F, 0xBF)]);
    let n851 = b.mk_concat(n145, n850);
    let n852 = b.mk_ranges_u8(&[(0x80, 0x87), (0x8F, 0x9F)]);
    let n853 = b.mk_concat(n148, n852);
    let n854 = b.mk_ranges_u8(&[(0xA0, 0xA1), (0xA3, 0xA4), (0xB0, 0xB1)]);
    let n855 = b.mk_concat(n151, n854);
    let n856 = b.mk_union(n853, n855);
    let n857 = b.mk_union(n851, n856);
    let n858 = b.mk_union(n849, n857);
    let n859 = b.mk_union(n847, n858);
    let n860 = b.mk_union(n845, n859);
    let n861 = b.mk_union(n843, n860);
    let n862 = b.mk_union(n842, n861);
    let n863 = b.mk_union(n840, n862);
    let n864 = b.mk_union(n839, n863);
    let n865 = b.mk_union(n837, n864);
    let n866 = b.mk_union(n836, n865);
    let n867 = b.mk_union(n834, n866);
    let n868 = b.mk_concat(n568, n867);
    let n869 = b.mk_ranges_u8(&[(0x97, 0x97), (0xA0, 0xA9), (0xAD, 0xAD), (0xB0, 0xB0)]);
    let n870 = b.mk_concat(n869, n407);
    let n871 = b.mk_ranges_u8(&[(0x80, 0x9E), (0xA0, 0xB2)]);
    let n872 = b.mk_concat(n871, n8);
    let n873 = b.mk_concat(n237, n268);
    let n874 = b.mk_ranges_u8(&[(0x80, 0x95), (0xBF, 0xBF)]);
    let n875 = b.mk_concat(n115, n874);
    let n876 = b.mk_concat(n118, n340);
    let n877 = b.mk_union(n875, n876);
    let n878 = b.mk_union(n873, n877);
    let n879 = b.mk_union(n872, n878);
    let n880 = b.mk_concat(n416, n879);
    let n881 = b.mk_ranges_u8(&[(0xB0, 0xB3), (0xB5, 0xBB), (0xBD, 0xBE)]);
    let n882 = b.mk_concat(n151, n881);
    let n883 = b.mk_concat(n225, n882);
    let n884 = b.mk_ranges_u8(&[(0x80, 0x83), (0x86, 0x8A), (0xB0, 0xB0)]);
    let n885 = b.mk_concat(n884, n8);
    let n886 = b.mk_ranges_u8(&[(0x80, 0xA2), (0xB2, 0xB2)]);
    let n887 = b.mk_concat(n334, n886);
    let n888 = b.mk_ranges_u8(&[(0x90, 0x92), (0x95, 0x95), (0xA4, 0xA7), (0xB0, 0xBF)]);
    let n889 = b.mk_concat(n337, n888);
    let n890 = b.mk_range_u8(0x80, 0xBB);
    let n891 = b.mk_concat(n204, n890);
    let n892 = b.mk_ranges_u8(&[(0x80, 0xAA), (0xB0, 0xBC)]);
    let n893 = b.mk_concat(n109, n892);
    let n894 = b.mk_ranges_u8(&[(0x80, 0x88), (0x90, 0x99), (0x9D, 0x9E)]);
    let n895 = b.mk_concat(n112, n894);
    let n896 = b.mk_union(n893, n895);
    let n897 = b.mk_union(n891, n896);
    let n898 = b.mk_union(n889, n897);
    let n899 = b.mk_union(n887, n898);
    let n900 = b.mk_union(n885, n899);
    let n901 = b.mk_concat(n228, n900);
    let n902 = b.mk_range_u8(0xB0, 0xB9);
    let n903 = b.mk_concat(n115, n902);
    let n904 = b.mk_concat(n142, n434);
    let n905 = b.mk_concat(n145, n718);
    let n906 = b.mk_union(n904, n905);
    let n907 = b.mk_union(n903, n906);
    let n908 = b.mk_concat(n231, n907);
    let n909 = b.mk_ranges_u8(&[(0xA5, 0xA9), (0xAD, 0xB2), (0xBB, 0xBF)]);
    let n910 = b.mk_concat(n337, n909);
    let n911 = b.mk_ranges_u8(&[(0x80, 0x82), (0x85, 0x8B), (0xAA, 0xAD)]);
    let n912 = b.mk_concat(n152, n911);
    let n913 = b.mk_range_u8(0x82, 0x84);
    let n914 = b.mk_concat(n198, n913);
    let n915 = b.mk_ranges_u8(&[(0x90, 0x90), (0x96, 0x99)]);
    let n916 = b.mk_concat(n915, n8);
    let n917 = b.mk_ranges_u8(&[(0x80, 0x94), (0x96, 0xBF)]);
    let n918 = b.mk_concat(n670, n917);
    let n919 = b.mk_ranges_u8(&[
        (0x80, 0x9C),
        (0x9E, 0x9F),
        (0xA2, 0xA2),
        (0xA5, 0xA6),
        (0xA9, 0xAC),
        (0xAE, 0xB9),
        (0xBB, 0xBB),
        (0xBD, 0xBF),
    ]);
    let n920 = b.mk_concat(n342, n919);
    let n921 = b.mk_ranges_u8(&[(0x80, 0x83), (0x85, 0xBF)]);
    let n922 = b.mk_concat(n345, n921);
    let n923 = b.mk_ranges_u8(&[
        (0x80, 0x85),
        (0x87, 0x8A),
        (0x8D, 0x94),
        (0x96, 0x9C),
        (0x9E, 0xB9),
        (0xBB, 0xBE),
    ]);
    let n924 = b.mk_concat(n562, n923);
    let n925 = b.mk_ranges_u8(&[(0x80, 0x84), (0x86, 0x86), (0x8A, 0x90), (0x92, 0xBF)]);
    let n926 = b.mk_concat(n565, n925);
    let n927 = b.mk_ranges_u8(&[(0x80, 0xA5), (0xA8, 0xBF)]);
    let n928 = b.mk_concat(n225, n927);
    let n929 = b.mk_ranges_u8(&[(0x80, 0x80), (0x82, 0x9A), (0x9C, 0xBA), (0xBC, 0xBF)]);
    let n930 = b.mk_concat(n228, n929);
    let n931 = b.mk_ranges_u8(&[(0x80, 0x94), (0x96, 0xB4), (0xB6, 0xBF)]);
    let n932 = b.mk_concat(n231, n931);
    let n933 = b.mk_ranges_u8(&[(0x80, 0x8E), (0x90, 0xAE), (0xB0, 0xBF)]);
    let n934 = b.mk_concat(n234, n933);
    let n935 = b.mk_ranges_u8(&[(0x80, 0x88), (0x8A, 0xA8), (0xAA, 0xBF)]);
    let n936 = b.mk_concat(n424, n935);
    let n937 = b.mk_ranges_u8(&[(0x80, 0x82), (0x84, 0x8B), (0x8E, 0xBF)]);
    let n938 = b.mk_concat(n237, n937);
    let n939 = b.mk_ranges_u8(&[(0x80, 0xB6), (0xBB, 0xBF)]);
    let n940 = b.mk_concat(n82, n939);
    let n941 = b.mk_ranges_u8(&[(0x80, 0xAC), (0xB5, 0xB5)]);
    let n942 = b.mk_concat(n85, n941);
    let n943 = b.mk_ranges_u8(&[(0x84, 0x84), (0x9B, 0x9F), (0xA1, 0xAF)]);
    let n944 = b.mk_concat(n88, n943);
    let n945 = b.mk_ranges_u8(&[(0x80, 0x9E), (0xA5, 0xAA)]);
    let n946 = b.mk_concat(n142, n945);
    let n947 = b.mk_union(n944, n946);
    let n948 = b.mk_union(n942, n947);
    let n949 = b.mk_union(n940, n948);
    let n950 = b.mk_union(n938, n949);
    let n951 = b.mk_union(n936, n950);
    let n952 = b.mk_union(n934, n951);
    let n953 = b.mk_union(n932, n952);
    let n954 = b.mk_union(n930, n953);
    let n955 = b.mk_union(n928, n954);
    let n956 = b.mk_union(n926, n955);
    let n957 = b.mk_union(n924, n956);
    let n958 = b.mk_union(n922, n957);
    let n959 = b.mk_union(n920, n958);
    let n960 = b.mk_union(n918, n959);
    let n961 = b.mk_union(n916, n960);
    let n962 = b.mk_union(n914, n961);
    let n963 = b.mk_union(n912, n962);
    let n964 = b.mk_union(n910, n963);
    let n965 = b.mk_concat(n234, n964);
    let n966 = b.mk_ranges_u8(&[
        (0x80, 0x86),
        (0x88, 0x98),
        (0x9B, 0xA1),
        (0xA3, 0xA4),
        (0xA6, 0xAA),
        (0xB0, 0xBF),
    ]);
    let n967 = b.mk_concat(n325, n966);
    let n968 = b.mk_concat(n189, n60);
    let n969 = b.mk_concat(n192, n216);
    let n970 = b.mk_ranges_u8(&[(0x80, 0xAC), (0xB0, 0xBD)]);
    let n971 = b.mk_concat(n334, n970);
    let n972 = b.mk_ranges_u8(&[(0x80, 0x89), (0x8E, 0x8E)]);
    let n973 = b.mk_concat(n337, n972);
    let n974 = b.mk_range_u8(0x90, 0xAE);
    let n975 = b.mk_concat(n201, n974);
    let n976 = b.mk_concat(n204, n833);
    let n977 = b.mk_range_u8(0x90, 0xB9);
    let n978 = b.mk_concat(n345, n977);
    let n979 = b.mk_range_u8(0x90, 0xBA);
    let n980 = b.mk_concat(n571, n979);
    let n981 = b.mk_ranges_u8(&[(0xA0, 0xA6), (0xA8, 0xAB), (0xAD, 0xAE), (0xB0, 0xBE)]);
    let n982 = b.mk_concat(n237, n981);
    let n983 = b.mk_ranges_u8(&[(0xA0, 0xA2), (0xA4, 0xA4)]);
    let n984 = b.mk_concat(n983, n8);
    let n985 = b.mk_ranges_u8(&[(0x80, 0x84), (0x90, 0x96)]);
    let n986 = b.mk_concat(n68, n985);
    let n987 = b.mk_ranges_u8(&[(0x80, 0x8B), (0x90, 0x99)]);
    let n988 = b.mk_concat(n73, n987);
    let n989 = b.mk_ranges_u8(&[
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
    let n990 = b.mk_concat(n130, n989);
    let n991 = b.mk_ranges_u8(&[
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
    let n992 = b.mk_concat(n133, n991);
    let n993 = b.mk_ranges_u8(&[
        (0x80, 0x89),
        (0x8B, 0x9B),
        (0xA1, 0xA3),
        (0xA5, 0xA9),
        (0xAB, 0xBB),
    ]);
    let n994 = b.mk_concat(n136, n993);
    let n995 = b.mk_union(n992, n994);
    let n996 = b.mk_union(n990, n995);
    let n997 = b.mk_union(n988, n996);
    let n998 = b.mk_union(n986, n997);
    let n999 = b.mk_union(n984, n998);
    let n1000 = b.mk_union(n982, n999);
    let n1001 = b.mk_union(n980, n1000);
    let n1002 = b.mk_union(n978, n1001);
    let n1003 = b.mk_union(n976, n1002);
    let n1004 = b.mk_union(n975, n1003);
    let n1005 = b.mk_union(n973, n1004);
    let n1006 = b.mk_union(n971, n1005);
    let n1007 = b.mk_union(n969, n1006);
    let n1008 = b.mk_union(n968, n1007);
    let n1009 = b.mk_union(n967, n1008);
    let n1010 = b.mk_concat(n424, n1009);
    let n1011 = b.mk_concat(n334, n392);
    let n1012 = b.mk_ranges_u8(&[(0x80, 0x89), (0x90, 0xA9), (0xB0, 0xBF)]);
    let n1013 = b.mk_concat(n337, n1012);
    let n1014 = b.mk_range_u8(0x80, 0x89);
    let n1015 = b.mk_concat(n152, n1014);
    let n1016 = b.mk_concat(n103, n902);
    let n1017 = b.mk_union(n1015, n1016);
    let n1018 = b.mk_union(n1013, n1017);
    let n1019 = b.mk_union(n1011, n1018);
    let n1020 = b.mk_concat(n237, n1019);
    let n1021 = b.mk_ranges_u8(&[(0x80, 0x9A), (0x9C, 0xBF)]);
    let n1022 = b.mk_concat(n1021, n8);
    let n1023 = b.mk_range_u8(0x80, 0x9F);
    let n1024 = b.mk_concat(n228, n1023);
    let n1025 = b.mk_union(n1022, n1024);
    let n1026 = b.mk_concat(n88, n1025);
    let n1027 = b.mk_ranges_u8(&[(0x80, 0x9B), (0x9D, 0x9F), (0xA1, 0xBF)]);
    let n1028 = b.mk_concat(n1027, n8);
    let n1029 = b.mk_concat(n231, n833);
    let n1030 = b.mk_concat(n59, n193);
    let n1031 = b.mk_union(n1029, n1030);
    let n1032 = b.mk_union(n1028, n1031);
    let n1033 = b.mk_concat(n91, n1032);
    let n1034 = b.mk_ranges_u8(&[(0x80, 0xB9), (0xBB, 0xBF)]);
    let n1035 = b.mk_concat(n1034, n8);
    let n1036 = b.mk_ranges_u8(&[(0x80, 0xA1), (0xB0, 0xBF)]);
    let n1037 = b.mk_concat(n136, n1036);
    let n1038 = b.mk_union(n1035, n1037);
    let n1039 = b.mk_concat(n94, n1038);
    let n1040 = b.mk_ranges_u8(&[(0x80, 0xAE), (0xB0, 0xB8)]);
    let n1041 = b.mk_concat(n1040, n8);
    let n1042 = b.mk_ranges_u8(&[(0x80, 0xA0), (0xB0, 0xBF)]);
    let n1043 = b.mk_concat(n103, n1042);
    let n1044 = b.mk_concat(n133, n474);
    let n1045 = b.mk_union(n1043, n1044);
    let n1046 = b.mk_union(n1041, n1045);
    let n1047 = b.mk_concat(n100, n1046);
    let n1048 = b.mk_range_u8(0xA0, 0xA7);
    let n1049 = b.mk_concat(n1048, n8);
    let n1050 = b.mk_concat(n82, n474);
    let n1051 = b.mk_union(n1049, n1050);
    let n1052 = b.mk_concat(n103, n1051);
    let n1053 = b.mk_ranges_u8(&[(0x80, 0x8C), (0x8E, 0xBF)]);
    let n1054 = b.mk_concat(n1053, n8);
    let n1055 = b.mk_ranges_u8(&[(0x80, 0x8A), (0x90, 0xBF)]);
    let n1056 = b.mk_concat(n210, n1055);
    let n1057 = b.mk_union(n1054, n1056);
    let n1058 = b.mk_concat(n109, n1057);
    let n1059 = b.mk_range_u8(0x80, 0x8D);
    let n1060 = b.mk_concat(n1059, n8);
    let n1061 = b.mk_concat(n213, n819);
    let n1062 = b.mk_union(n1060, n1061);
    let n1063 = b.mk_concat(n112, n1062);
    let n1064 = b.mk_union(n1058, n1063);
    let n1065 = b.mk_union(n1052, n1064);
    let n1066 = b.mk_union(n1047, n1065);
    let n1067 = b.mk_union(n1039, n1066);
    let n1068 = b.mk_union(n1033, n1067);
    let n1069 = b.mk_union(n1026, n1068);
    let n1070 = b.mk_union(n1020, n1069);
    let n1071 = b.mk_union(n1010, n1070);
    let n1072 = b.mk_union(n965, n1071);
    let n1073 = b.mk_union(n908, n1072);
    let n1074 = b.mk_union(n901, n1073);
    let n1075 = b.mk_union(n883, n1074);
    let n1076 = b.mk_union(n880, n1075);
    let n1077 = b.mk_union(n870, n1076);
    let n1078 = b.mk_union(n868, n1077);
    let n1079 = b.mk_union(n832, n1078);
    let n1080 = b.mk_union(n825, n1079);
    let n1081 = b.mk_union(n817, n1080);
    let n1082 = b.mk_union(n801, n1081);
    let n1083 = b.mk_union(n669, n1082);
    let n1084 = b.mk_concat(n535, n1083);
    let n1085 = b.mk_range_u8(0xF3, 0xF3);
    let n1086 = b.mk_range_u8(0x84, 0x86);
    let n1087 = b.mk_concat(n1086, n8);
    let n1088 = b.mk_concat(n391, n819);
    let n1089 = b.mk_union(n1087, n1088);
    let n1090 = b.mk_concat(n59, n1089);
    let n1091 = b.mk_concat(n1085, n1090);
    let n1092 = b.mk_union(n1084, n1091);
    let n1093 = b.mk_union(n534, n1092);
    let n1094 = b.mk_union(n482, n1093);
    let n1095 = b.mk_union(n472, n1094);
    let n1096 = b.mk_union(n408, n1095);
    let n1097 = b.mk_union(n405, n1096);
    let n1098 = b.mk_union(n401, n1097);
    let n1099 = b.mk_union(n376, n1098);
    let n1100 = b.mk_union(n323, n1099);
    let n1101 = b.mk_union(n185, n1100);
    let n1102 = b.mk_union(n57, n1101);
    let n1103 = b.mk_union(n54, n1102);
    let n1104 = b.mk_union(n51, n1103);
    let n1105 = b.mk_union(n48, n1104);
    let n1106 = b.mk_union(n45, n1105);
    let n1107 = b.mk_union(n42, n1106);
    let n1108 = b.mk_union(n39, n1107);
    let n1109 = b.mk_union(n36, n1108);
    let n1110 = b.mk_union(n33, n1109);
    let n1111 = b.mk_union(n30, n1110);
    let n1112 = b.mk_union(n27, n1111);
    let n1113 = b.mk_union(n24, n1112);
    let n1114 = b.mk_union(n21, n1113);
    let n1115 = b.mk_union(n18, n1114);
    let n1116 = b.mk_union(n15, n1115);
    let n1117 = b.mk_union(n12, n1116);
    let n1118 = b.mk_union(n9, n1117);
    let n1119 = b.mk_union(n6, n1118);
    let n1120 = b.mk_union(n3, n1119);
    let n1121 = b.mk_union(n0, n1120);
    n1121
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
    let n11 = b.mk_ranges_u8(&[
        (0xA5, 0xA5),
        (0xA7, 0xA7),
        (0xA9, 0xA9),
        (0xAB, 0xAB),
        (0xAD, 0xAD),
        (0xAF, 0xAF),
        (0xB1, 0xB1),
        (0xB3, 0xB3),
        (0xB5, 0xB5),
        (0xB7, 0xB7),
    ]);
    let n12 = b.mk_range_u8(0xA6, 0xAF);
    let n13 = b.mk_concat(n11, n12);
    let n14 = b.mk_ranges_u8(&[(0xB9, 0xB9), (0xBB, 0xBB)]);
    let n15 = b.mk_range_u8(0x90, 0x99);
    let n16 = b.mk_concat(n14, n15);
    let n17 = b.mk_range_u8(0xBC, 0xBC);
    let n18 = b.mk_concat(n17, n2);
    let n19 = b.mk_union(n16, n18);
    let n20 = b.mk_union(n13, n19);
    let n21 = b.mk_concat(n10, n20);
    let n22 = b.mk_range_u8(0xE1, 0xE1);
    let n23 = b.mk_range_u8(0x81, 0x81);
    let n24 = b.mk_concat(n23, n8);
    let n25 = b.mk_ranges_u8(&[(0x82, 0x82), (0xA0, 0xA0), (0xA7, 0xA7), (0xAD, 0xAD)]);
    let n26 = b.mk_concat(n25, n15);
    let n27 = b.mk_range_u8(0x9F, 0x9F);
    let n28 = b.mk_concat(n27, n2);
    let n29 = b.mk_range_u8(0xA5, 0xA5);
    let n30 = b.mk_range_u8(0x86, 0x8F);
    let n31 = b.mk_concat(n29, n30);
    let n32 = b.mk_ranges_u8(&[(0xAA, 0xAA), (0xB1, 0xB1)]);
    let n33 = b.mk_ranges_u8(&[(0x80, 0x89), (0x90, 0x99)]);
    let n34 = b.mk_concat(n32, n33);
    let n35 = b.mk_range_u8(0xAE, 0xAE);
    let n36 = b.mk_concat(n35, n5);
    let n37 = b.mk_union(n34, n36);
    let n38 = b.mk_union(n31, n37);
    let n39 = b.mk_union(n28, n38);
    let n40 = b.mk_union(n26, n39);
    let n41 = b.mk_union(n24, n40);
    let n42 = b.mk_concat(n22, n41);
    let n43 = b.mk_range_u8(0xEA, 0xEA);
    let n44 = b.mk_range_u8(0x98, 0x98);
    let n45 = b.mk_concat(n44, n2);
    let n46 = b.mk_ranges_u8(&[(0xA3, 0xA3), (0xA9, 0xA9)]);
    let n47 = b.mk_concat(n46, n15);
    let n48 = b.mk_range_u8(0xA4, 0xA4);
    let n49 = b.mk_concat(n48, n8);
    let n50 = b.mk_range_u8(0xA7, 0xA7);
    let n51 = b.mk_ranges_u8(&[(0x90, 0x99), (0xB0, 0xB9)]);
    let n52 = b.mk_concat(n50, n51);
    let n53 = b.mk_range_u8(0xAF, 0xAF);
    let n54 = b.mk_concat(n53, n5);
    let n55 = b.mk_union(n52, n54);
    let n56 = b.mk_union(n49, n55);
    let n57 = b.mk_union(n47, n56);
    let n58 = b.mk_union(n45, n57);
    let n59 = b.mk_concat(n43, n58);
    let n60 = b.mk_range_u8(0xEF, 0xEF);
    let n61 = b.mk_concat(n17, n15);
    let n62 = b.mk_concat(n60, n61);
    let n63 = b.mk_range_u8(0xF0, 0xF0);
    let n64 = b.mk_range_u8(0x90, 0x90);
    let n65 = b.mk_range_u8(0x92, 0x92);
    let n66 = b.mk_concat(n65, n2);
    let n67 = b.mk_range_u8(0xB4, 0xB4);
    let n68 = b.mk_concat(n67, n5);
    let n69 = b.mk_range_u8(0xB5, 0xB5);
    let n70 = b.mk_concat(n69, n8);
    let n71 = b.mk_union(n68, n70);
    let n72 = b.mk_union(n66, n71);
    let n73 = b.mk_concat(n64, n72);
    let n74 = b.mk_range_u8(0x91, 0x91);
    let n75 = b.mk_concat(n23, n12);
    let n76 = b.mk_ranges_u8(&[(0x83, 0x83), (0x8B, 0x8B), (0x9C, 0x9C), (0xAF, 0xAF)]);
    let n77 = b.mk_concat(n76, n5);
    let n78 = b.mk_range_u8(0x84, 0x84);
    let n79 = b.mk_range_u8(0xB6, 0xBF);
    let n80 = b.mk_concat(n78, n79);
    let n81 = b.mk_ranges_u8(&[
        (0x87, 0x87),
        (0x91, 0x91),
        (0x93, 0x93),
        (0x99, 0x99),
        (0xA5, 0xA5),
        (0xB1, 0xB1),
        (0xB5, 0xB5),
        (0xBD, 0xBD),
    ]);
    let n82 = b.mk_concat(n81, n15);
    let n83 = b.mk_range_u8(0x9B, 0x9B);
    let n84 = b.mk_ranges_u8(&[(0x80, 0x89), (0x90, 0xA3)]);
    let n85 = b.mk_concat(n83, n84);
    let n86 = b.mk_ranges_u8(&[(0xA3, 0xA3), (0xB6, 0xB6)]);
    let n87 = b.mk_concat(n86, n2);
    let n88 = b.mk_union(n85, n87);
    let n89 = b.mk_union(n82, n88);
    let n90 = b.mk_union(n80, n89);
    let n91 = b.mk_union(n77, n90);
    let n92 = b.mk_union(n75, n91);
    let n93 = b.mk_concat(n74, n92);
    let n94 = b.mk_range_u8(0x96, 0x96);
    let n95 = b.mk_ranges_u8(&[(0x84, 0x84), (0xB5, 0xB5)]);
    let n96 = b.mk_concat(n95, n5);
    let n97 = b.mk_range_u8(0xA9, 0xA9);
    let n98 = b.mk_concat(n97, n2);
    let n99 = b.mk_range_u8(0xAB, 0xAB);
    let n100 = b.mk_concat(n99, n8);
    let n101 = b.mk_range_u8(0xAD, 0xAD);
    let n102 = b.mk_concat(n101, n15);
    let n103 = b.mk_union(n100, n102);
    let n104 = b.mk_union(n98, n103);
    let n105 = b.mk_union(n96, n104);
    let n106 = b.mk_concat(n94, n105);
    let n107 = b.mk_range_u8(0x9C, 0x9C);
    let n108 = b.mk_range_u8(0xB3, 0xB3);
    let n109 = b.mk_concat(n108, n5);
    let n110 = b.mk_concat(n107, n109);
    let n111 = b.mk_range_u8(0x9D, 0x9D);
    let n112 = b.mk_range_u8(0x8E, 0xBF);
    let n113 = b.mk_concat(n27, n112);
    let n114 = b.mk_concat(n111, n113);
    let n115 = b.mk_range_u8(0x9E, 0x9E);
    let n116 = b.mk_range_u8(0x85, 0x85);
    let n117 = b.mk_concat(n116, n8);
    let n118 = b.mk_ranges_u8(&[(0x8B, 0x8B), (0x93, 0x93)]);
    let n119 = b.mk_concat(n118, n5);
    let n120 = b.mk_range_u8(0x97, 0x97);
    let n121 = b.mk_range_u8(0xB1, 0xBA);
    let n122 = b.mk_concat(n120, n121);
    let n123 = b.mk_concat(n29, n15);
    let n124 = b.mk_union(n122, n123);
    let n125 = b.mk_union(n119, n124);
    let n126 = b.mk_union(n117, n125);
    let n127 = b.mk_concat(n115, n126);
    let n128 = b.mk_concat(n27, n54);
    let n129 = b.mk_union(n127, n128);
    let n130 = b.mk_union(n114, n129);
    let n131 = b.mk_union(n110, n130);
    let n132 = b.mk_union(n106, n131);
    let n133 = b.mk_union(n93, n132);
    let n134 = b.mk_union(n73, n133);
    let n135 = b.mk_concat(n63, n134);
    let n136 = b.mk_union(n62, n135);
    let n137 = b.mk_union(n59, n136);
    let n138 = b.mk_union(n42, n137);
    let n139 = b.mk_union(n21, n138);
    let n140 = b.mk_union(n9, n139);
    let n141 = b.mk_union(n6, n140);
    let n142 = b.mk_union(n3, n141);
    let n143 = b.mk_union(n0, n142);
    n143
}

pub fn build_space_class_full(b: &mut RegexBuilder) -> NodeId {
    let n0 = b.mk_ranges_u8(&[(0x09, 0x0D), (0x20, 0x20)]);
    let n1 = b.mk_range_u8(0xC2, 0xC2);
    let n2 = b.mk_ranges_u8(&[(0x85, 0x85), (0xA0, 0xA0)]);
    let n3 = b.mk_concat(n1, n2);
    let n4 = b.mk_range_u8(0xE1, 0xE1);
    let n5 = b.mk_range_u8(0x9A, 0x9A);
    let n6 = b.mk_range_u8(0x80, 0x80);
    let n7 = b.mk_concat(n5, n6);
    let n8 = b.mk_concat(n4, n7);
    let n9 = b.mk_range_u8(0xE2, 0xE2);
    let n10 = b.mk_ranges_u8(&[(0x80, 0x8A), (0xA8, 0xA9), (0xAF, 0xAF)]);
    let n11 = b.mk_concat(n6, n10);
    let n12 = b.mk_range_u8(0x81, 0x81);
    let n13 = b.mk_range_u8(0x9F, 0x9F);
    let n14 = b.mk_concat(n12, n13);
    let n15 = b.mk_union(n11, n14);
    let n16 = b.mk_concat(n9, n15);
    let n17 = b.mk_range_u8(0xE3, 0xE3);
    let n18 = b.mk_concat(n6, n6);
    let n19 = b.mk_concat(n17, n18);
    let n20 = b.mk_union(n16, n19);
    let n21 = b.mk_union(n8, n20);
    let n22 = b.mk_union(n3, n21);
    let n23 = b.mk_union(n0, n22);
    n23
}
