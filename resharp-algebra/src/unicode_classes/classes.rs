use crate::{RegexBuilder, NodeId};

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
let n11 = b.mk_ranges_u8(&[(0x80, 0x81), (0x86, 0x91), (0xA0, 0xA4), (0xAC, 0xAC), (0xAE, 0xAE)]);
let n12 = b.mk_concat(n10, n11);
let n13 = b.mk_range_u8(0xCC, 0xCC);
let n14 = b.mk_concat(n13, n8);
let n15 = b.mk_range_u8(0xCD, 0xCD);
let n16 = b.mk_ranges_u8(&[(0x80, 0xB4), (0xB6, 0xB7), (0xBA, 0xBD), (0xBF, 0xBF)]);
let n17 = b.mk_concat(n15, n16);
let n18 = b.mk_range_u8(0xCE, 0xCE);
let n19 = b.mk_ranges_u8(&[(0x86, 0x86), (0x88, 0x8A), (0x8C, 0x8C), (0x8E, 0xA1), (0xA3, 0xBF)]);
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
let n41 = b.mk_ranges_u8(&[(0x81, 0x82), (0x84, 0x85), (0x87, 0x87), (0x90, 0xAA), (0xAF, 0xB2)]);
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
let n52 = b.mk_ranges_u8(&[(0x80, 0x93), (0x95, 0x9C), (0x9F, 0xA8), (0xAA, 0xBC), (0xBF, 0xBF)]);
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


