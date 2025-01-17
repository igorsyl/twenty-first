use rusty_leveldb::{WriteBatch, DB};
use serde::{de::DeserializeOwned, Serialize};
use std::marker::PhantomData;

/// This is the key for the storage of the length of the vector
/// Due to a bug in rusty-levelDB we use 1 byte, not 0 bytes to store the length
/// of the vector. Cf. https://github.com/dermesser/leveldb-rs/issues/16
/// This is OK to do as long as collide with a key. Since the keys for indices
/// are all 16 bytes long when using 128s, then its OK to use a 1-byte key here.
const LENGTH_KEY: [u8; 1] = [0];
type IndexType = u64;
const INDEX_ZERO: IndexType = 0;

pub struct DatabaseVector<T: Serialize + DeserializeOwned> {
    db: DB,
    _type: PhantomData<T>,
}

impl<T: Serialize + DeserializeOwned> DatabaseVector<T> {
    fn set_length(&mut self, length: IndexType) {
        let length_as_bytes = bincode::serialize(&length).unwrap();
        self.db
            .put(&LENGTH_KEY, &length_as_bytes)
            .expect("Length write must succeed");
    }

    fn delete(&mut self, index: IndexType) {
        let index_as_bytes = bincode::serialize(&index).unwrap();
        self.db
            .delete(&index_as_bytes)
            .expect("Deleting element must succeed");
    }

    /// Return true if the database vector looks empty. Used for sanity check when creating
    /// a new database vector.
    fn attempt_verify_empty(&mut self) -> bool {
        let index_bytes: Vec<u8> = bincode::serialize(&INDEX_ZERO).unwrap();
        self.db.get(&index_bytes).is_none()
    }

    pub fn is_empty(&mut self) -> bool {
        self.len() == 0
    }

    pub fn flush(&mut self) {
        self.db.flush().expect("Flush must succeed.")
    }

    pub fn len(&mut self) -> IndexType {
        let length_as_bytes = self.db.get(&LENGTH_KEY).expect("Length must exist");
        bincode::deserialize(&length_as_bytes).unwrap()
    }

    /// given a database containing a database vector, restore it into a database vector struct
    pub fn restore(db: DB) -> Self {
        let mut ret = Self {
            _type: PhantomData,
            db,
        };

        // sanity check to verify that the length is set
        let _dummy_res = ret.len();
        ret
    }

    pub fn overwrite_with_vec(&mut self, new_vector: Vec<T>) {
        let old_length = self.len();
        let new_length = new_vector.len() as IndexType;
        self.set_length(new_length);

        let mut batch_write = WriteBatch::new();
        for (index, val) in new_vector.into_iter().enumerate() {
            // Notice that `index` has to be cast to the type of the index for this data structure.
            // Otherwise this function will create a corrupted database.
            let index_bytes: Vec<u8> = bincode::serialize(&(index as IndexType)).unwrap();
            let value_bytes: Vec<u8> = bincode::serialize(&val).unwrap();
            batch_write.put(&index_bytes, &value_bytes);
        }

        for index in new_length..old_length {
            let index_bytes: Vec<u8> = bincode::serialize(&index).unwrap();
            batch_write.delete(&index_bytes);
        }

        self.db
            .write(batch_write, true)
            .expect("Failed to batch-write to database in overwrite_with_vec");
    }

    /// Create a new, empty database vector
    pub fn new(db: DB) -> Self {
        let mut ret = DatabaseVector {
            db,
            _type: PhantomData,
        };
        // TODO: It might be possible to check this more rigorously using a DBIterator
        assert!(
            ret.attempt_verify_empty(),
            "Database must be empty when instantiating database vector with `new`"
        );
        ret.set_length(0);

        ret
    }

    pub fn get(&mut self, index: IndexType) -> T {
        debug_assert!(
            self.len() > index,
            "Cannot get outside of length. Length: {}, index: {}",
            self.len(),
            index
        );
        let index_bytes: Vec<u8> = bincode::serialize(&index).unwrap();
        let elem_as_bytes = self.db.get(&index_bytes).unwrap();
        bincode::deserialize(&elem_as_bytes).unwrap()
    }

    pub fn set(&mut self, index: IndexType, value: T) {
        debug_assert!(
            self.len() > index,
            "Cannot set outside of length. Length: {}, index: {}",
            self.len(),
            index
        );
        let index_bytes: Vec<u8> = bincode::serialize(&index).unwrap();
        let value_bytes: Vec<u8> = bincode::serialize(&value).unwrap();
        self.db.put(&index_bytes, &value_bytes).unwrap();
    }

    pub fn batch_set(&mut self, indices_and_vals: &[(IndexType, T)]) {
        let indices: Vec<IndexType> = indices_and_vals.iter().map(|(index, _)| *index).collect();
        let length = self.len();
        assert!(
            indices.iter().all(|index| *index < length),
            "All indices must be lower than length of vector. Got: {indices:?}"
        );
        let mut batch_write = WriteBatch::new();
        for (index, val) in indices_and_vals.iter() {
            let index_bytes: Vec<u8> = bincode::serialize(index).unwrap();
            let value_bytes: Vec<u8> = bincode::serialize(val).unwrap();
            batch_write.put(&index_bytes, &value_bytes);
        }

        self.db
            .write(batch_write, true)
            .expect("Failed to batch-write to database in batch_set");
    }

    pub fn pop(&mut self) -> Option<T> {
        match self.len() {
            0 => None,
            length => {
                let element = self.get(length - 1);
                self.delete(length - 1);
                self.set_length(length - 1);
                Some(element)
            }
        }
    }

    pub fn push(&mut self, value: T) {
        let length = self.len();
        let index_bytes = bincode::serialize(&length).unwrap();
        let value_bytes = bincode::serialize(&value).unwrap();
        self.db.put(&index_bytes, &value_bytes).unwrap();
        self.set_length(length + 1);
    }

    /// Dispose of the vector and return the database. You should probably only use this for testing.
    pub fn extract_db(self) -> DB {
        self.db
    }
}

#[cfg(test)]
mod database_vector_tests {
    use super::*;
    use rusty_leveldb::DB;

    fn test_constructor() -> DatabaseVector<u64> {
        let opt = rusty_leveldb::in_memory();
        let db = DB::open("mydatabase", opt).unwrap();
        DatabaseVector::new(db)
    }

    #[test]
    fn push_pop_test() {
        let mut db_vector = test_constructor();
        assert_eq!(0, db_vector.len());
        assert!(db_vector.is_empty());

        // pop an element and verify that `None` is returns
        assert!(db_vector.pop().is_none());
        assert_eq!(0, db_vector.len());
        assert!(db_vector.is_empty());

        // push two elements and check length and values
        db_vector.push(14442);
        db_vector.push(5558999);
        assert_eq!(14442, db_vector.get(0));
        assert_eq!(5558999, db_vector.get(1));
        assert_eq!(2, db_vector.len());

        // Set a value to verify that `set` works
        db_vector.set(1, 4);
        assert_eq!(4, db_vector.get(1));

        // Verify that `pop` works
        assert_eq!(Some(4), db_vector.pop());
        assert_eq!(1, db_vector.len());
        assert_eq!(Some(14442), db_vector.pop());
        assert_eq!(0, db_vector.len());
        assert!(db_vector.is_empty());
    }

    #[test]
    fn overwrite_with_vec_test() {
        let mut db_vector = test_constructor();
        for _ in 0..10 {
            db_vector.push(17);
        }

        // Verify that shortening the vector works
        let mut new_vector_values: Vec<u64> = (200..202).collect();
        db_vector.overwrite_with_vec(new_vector_values);
        assert_eq!(2, db_vector.len());
        assert_eq!(200, db_vector.get(0));

        // Verify that increasing the vector works
        new_vector_values = (200..350).collect();
        db_vector.overwrite_with_vec(new_vector_values);
        assert_eq!(150, db_vector.len());
        assert_eq!(200, db_vector.get(0));
        assert_eq!(300, db_vector.get(100));
    }

    #[test]
    fn batch_set_test() {
        let mut db_vector = test_constructor();
        for _ in 0..100 {
            db_vector.push(17);
        }

        // Batch-write and verify that the values are set correctly
        db_vector.batch_set(&[(40, 4040), (41, 4141), (44, 4444)]);
        assert_eq!(4040, db_vector.get(40));
        assert_eq!(4141, db_vector.get(41));
        assert_eq!(4444, db_vector.get(44));
        assert_eq!(17, db_vector.get(39));

        let new_vector_values: Vec<u64> = (200..202).collect();
        println!("new_vector_values = {new_vector_values:?}");
        db_vector.overwrite_with_vec(new_vector_values);
        assert_eq!(2, db_vector.len());
        assert_eq!(200, db_vector.get(0));
    }

    #[test]
    fn push_many_test() {
        let mut db_vector = test_constructor();
        for _ in 0..1000 {
            db_vector.push(17);
        }

        assert_eq!(1000, db_vector.len());
    }

    #[should_panic = "Cannot get outside of length. Length: 0, index: 0"]
    #[test]
    fn panic_on_index_out_of_range_empty_test() {
        let mut db_vector = test_constructor();
        db_vector.get(0);
    }

    #[should_panic = "Cannot get outside of length. Length: 1, index: 1"]
    #[test]
    fn panic_on_index_out_of_range_length_one_test() {
        let mut db_vector = test_constructor();
        db_vector.push(5558999);
        db_vector.get(1);
    }

    #[should_panic = "Cannot set outside of length. Length: 1, index: 1"]
    #[test]
    fn panic_on_index_out_of_range_length_one_set_test() {
        let mut db_vector = test_constructor();
        db_vector.push(5558999);
        db_vector.set(1, 14);
    }

    #[test]
    fn restore_test() {
        // Verify that we can restore a database vector object from a database object
        let mut db_vector = test_constructor();
        assert!(db_vector.is_empty());
        let extracted_db = db_vector.db;
        let mut new_db_vector: DatabaseVector<u64> = DatabaseVector::restore(extracted_db);
        assert!(new_db_vector.is_empty());
    }

    #[test]
    fn index_zero_test() {
        // Verify that index zero does not overwrite the stored length
        let mut db_vector = test_constructor();
        db_vector.push(17);
        assert_eq!(1, db_vector.len());
        assert_eq!(17u64, db_vector.get(0));
        assert_eq!(17u64, db_vector.pop().unwrap());
        assert_eq!(0, db_vector.len());
        assert!(db_vector.is_empty());
    }
}
