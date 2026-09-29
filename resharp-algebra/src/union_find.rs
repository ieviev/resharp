pub struct UF {
    root: Vec<usize>,
    data: Vec<usize>,
    sibling: Vec<usize>,
}

impl UF {
    pub fn new() -> UF {
        UF {
            root: vec![],
            data: vec![],
            sibling: vec![],
        }
    }

    #[inline]
    pub fn new_key(&mut self) -> usize {
        let key = self.data.len();
        self.root.push(key);
        self.sibling.push(key);
        self.data.push(key);
        key
    }

    #[inline]
    pub fn union(&mut self, left: usize, right: usize) -> bool {
        let lkey = self.find(left);
        let rkey = self.find(right);
        if lkey == rkey {
            return false;
        }
        let (root, child_root, last) = if left < right {
            (lkey, rkey, self.data[lkey])
        } else {
            (rkey, lkey, self.data[rkey])
        };
        self.sibling[last] = child_root;
        let mut elem = child_root;
        while self.sibling[elem] != elem {
            self.root[elem] = root;
            elem = self.sibling[elem];
        }
        self.root[elem] = root;
        self.data[root] = elem;
        true
    }

    #[inline(always)]
    pub fn find(&mut self, key: usize) -> usize {
        self.root[key]
    }
}

impl Default for UF {
    fn default() -> Self {
        Self::new()
    }
}
