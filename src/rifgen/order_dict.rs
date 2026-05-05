use std::{borrow::Borrow, collections::HashMap, hash::Hash};

/// Ordered dictionnary using both HashMap and Vec
/// The Hashmap contains keys with value's index,
/// and values are stored in order that items are added
#[derive(Clone, Debug, PartialEq)]
pub struct OrderDict<K,V> where K: Eq+Hash {
    keys: HashMap<K,usize>,
    values: Vec<V>
}

impl<K,V> OrderDict<K,V>
    where K: Eq + Hash
{

    /// Create an empty dictionnary
    pub fn new() -> Self {
        OrderDict { keys: HashMap::new(), values: Vec::new() }
    }

    /// Create an empty dictionnary, pre-allocating size for both HashMap and Vec
    pub fn with_capacity(n: usize) -> Self {
        OrderDict { keys: HashMap::with_capacity(n), values: Vec::with_capacity(n) }
    }

    /// Check if a key exists
    pub fn contains_key<Q>(&self, k: &Q) -> bool
    where K: Borrow<Q>, Q: Hash + Eq + ?Sized
    {
        self.keys.contains_key(k)
    }

    /// Insert a new item (pair key/value)
    pub fn insert(&mut self, k: K, v: V) {
        match self.keys.get(&k) {
            Some(i) => self.values[*i] = v,
            None => {
                self.keys.insert(k, self.values.len());
                self.values.push(v);
            }
        }
    }

    /// Clear all items
    pub fn clear(&mut self) {
        self.values.clear();
        self.keys.clear();
    }

    /// Return number of elements stored in the dictionnary
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Return true when dictionnary contains no key
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Return value of a key if it exists
    pub fn get<Q>(&self, k: &Q) -> Option<&V>
    where K: Borrow<Q>, Q: Hash + Eq + ?Sized
    {
        let i = self.keys.get(k)?;
        Some(&self.values[*i])
    }

    /// Return mutable access to the last value
    pub fn last_mut(&mut self) -> Option<&mut V> {
        self.values.last_mut()
    }

    /// Remove a key if it exist
    /// Note: does not remove internal value
    pub fn del_key(&mut self, k: &K) {
        self.keys.remove_entry(k);
    }

    /// Iterator over all values
    /// Note: still return value of deleted keys
    pub fn values(&self) -> OrderedDictIterV<'_, V> {
        OrderedDictIterV {
            values: &self.values,
            index: 0
        }
    }

    /// Iterator over items of the dictionnary, in the order they were inserted
    pub fn items(&self) -> OrderedDictIterKv<'_, K,V> {
        OrderedDictIterKv {
            dict: self,
            index: 0
        }
    }

    /// Return a mutable reference associated with a key
    /// If the key did not exist, create one with default value
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

    /// Return mutable reference to the value corresponding to a key (if it exists)
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where K: Borrow<Q>, Q: Hash + Eq + ?Sized
    {
        match self.keys.get(key) {
            Some(i) => Some(&mut self.values[*i]),
            None => None
        }

    }

    /// Return iterator over exisiting keys
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.keys.keys()
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
        let mut ki = self.dict.keys.iter().find(|&(_,v)| *v==self.index);
        while ki.is_none() {
            self.index += 1;
            ki = self.dict.keys.iter().find(|&(_,v)| *v==self.index);
            if self.index >= self.dict.values.len() {
                return None;
            }
        }
        let v = self.dict.values.get(self.index)?;
        let k = ki.unwrap().0;
        self.index += 1;
        Some((k,v))
    }
}
