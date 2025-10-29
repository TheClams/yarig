use std::{borrow::Borrow, collections::HashMap, hash::Hash};

#[derive(Clone, Debug, PartialEq)]
pub struct OrderDict<K,V> where K: Eq+Hash {
    keys: HashMap<K,usize>,
    values: Vec<V>
}

impl<K,V> OrderDict<K,V>
    where K: Eq + Hash
{

    pub fn new() -> Self {
        OrderDict { keys: HashMap::new(), values: Vec::new() }
    }

    pub fn with_capacity(n: usize) -> Self {
        OrderDict { keys: HashMap::with_capacity(n), values: Vec::with_capacity(n) }
    }

    pub fn contains_key(&self, k: &K) -> bool{
        self.keys.contains_key(k)
    }

    pub fn insert(&mut self, k: K, v: V) {
        match self.keys.get(&k) {
            Some(i) => self.values[*i] = v,
            None => {
                self.keys.insert(k, self.values.len());
                self.values.push(v);
            }
        }
    }

    pub fn clear(&mut self) {
        self.values.clear();
        self.keys.clear();
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn get(&self, k: &K) -> Option<&V> {
        let i = self.keys.get(k)?;
        Some(&self.values[*i])
    }

    pub fn last_mut(&mut self) -> Option<&mut V> {
        self.values.last_mut()
    }

    pub fn values(&self) -> OrderedDictIterV<'_, V> {
        OrderedDictIterV {
            values: &self.values,
            index: 0
        }
    }

    pub fn items(&self) -> OrderedDictIterKv<'_, K,V> {
        OrderedDictIterKv {
            dict: self,
            index: 0
        }
    }

    pub fn entry(&mut self, key: &K) -> &mut V
    where K: Clone, V: Default {
        let idx = match self.keys.get(key) {
            Some(i) => *i,
            None => {
                self.insert(key.clone(), V::default());
                self.values.len() - 1
            }
        };
        &mut self.values[idx]
    }

    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where K: Borrow<Q>, Q: Hash + Eq + ?Sized
    {
        match self.keys.get(key) {
            Some(i) => Some(&mut self.values[*i]),
            None => None
        }

    }

}

impl<K,V> Default for OrderDict<K,V> where K: Eq + Hash {
    fn default() -> Self {
        Self::new()
    }
}


//-----------------------------------------------------------------------------
// Implement iterator on value only
pub struct OrderedDictIterV<'a,V> {
    values: &'a Vec<V>,
    index: usize
}

impl<'a,V> Iterator for OrderedDictIterV<'a,V> {
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
        let v = self.values.get(self.index);
        self.index += 1;
        v
    }
}


//-----------------------------------------------------------------------------
// Implement iterator on Key/Value
pub struct OrderedDictIterKv<'a,K,V> where K: Eq+Hash {
    dict: &'a OrderDict<K,V>,
    index: usize
}

impl<'a,K,V> Iterator for OrderedDictIterKv<'a,K,V> where K: Eq+Hash  {
    type Item = (&'a K,&'a V);

    fn next(&mut self) -> Option<Self::Item> {
        let v = self.dict.values.get(self.index)?;
        let k = self.dict.keys.iter().find(|&(_,v)| *v==self.index).unwrap().0;
        self.index += 1;
        Some((k,v))
    }
}
