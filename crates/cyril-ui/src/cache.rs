use std::collections::HashMap;

/// A simple hash-keyed cache with insertion-order eviction.
///
/// When capacity is reached, the oldest half of entries are discarded.
/// Values are returned by cloning, so `V` must implement `Clone`.
pub struct HashCache<V> {
    map: HashMap<u64, V>,
    order: Vec<u64>,
    capacity: usize,
}

impl<V: Clone> HashCache<V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: Vec::new(),
            capacity,
        }
    }

    pub fn get(&self, key: u64) -> Option<&V> {
        self.map.get(&key)
    }

    pub fn insert(&mut self, key: u64, value: V) {
        if self.map.len() >= self.capacity {
            let drain_count = self.order.len() / 2;
            for evicted in self.order.drain(..drain_count) {
                self.map.remove(&evicted);
            }
        }
        if self.map.insert(key, value).is_none() {
            self.order.push(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_returns_none_for_missing_key() {
        let cache: HashCache<String> = HashCache::new(4);
        assert!(cache.get(42).is_none());
    }

    #[test]
    fn insert_and_get_returns_value() {
        let mut cache = HashCache::new(4);
        cache.insert(1, "hello".to_string());
        assert_eq!(cache.get(1), Some(&"hello".to_string()));
    }

    #[test]
    fn evicts_oldest_half_when_full() {
        let mut cache = HashCache::new(4);
        cache.insert(1, "a".to_string());
        cache.insert(2, "b".to_string());
        cache.insert(3, "c".to_string());
        cache.insert(4, "d".to_string());
        // Cache is at capacity. Next insert triggers eviction of oldest 2.
        cache.insert(5, "e".to_string());

        // Keys 1 and 2 should be evicted
        assert!(cache.get(1).is_none());
        assert!(cache.get(2).is_none());
        // Keys 3, 4, 5 should remain
        assert_eq!(cache.get(3), Some(&"c".to_string()));
        assert_eq!(cache.get(4), Some(&"d".to_string()));
        assert_eq!(cache.get(5), Some(&"e".to_string()));
    }

    #[test]
    fn duplicate_insert_does_not_duplicate_order() {
        let mut cache = HashCache::new(4);
        cache.insert(1, "a".to_string());
        cache.insert(1, "a-updated".to_string());
        cache.insert(2, "b".to_string());
        cache.insert(3, "c".to_string());
        cache.insert(4, "d".to_string());
        // Cache at capacity with 4 unique keys. Insert should evict oldest half (2 from order).
        cache.insert(5, "e".to_string());

        // Key 1 was inserted first — should be evicted
        assert!(cache.get(1).is_none());
    }
}
