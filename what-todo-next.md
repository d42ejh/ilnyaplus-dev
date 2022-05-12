# what to do next
Implement download features and test upload-download

# TODO
- [ ] file sha hash in root block
- [ ] load daemon serve address from config file.
- [ ] Implement resumeable task for encoding, downloading.
- [ ] use tower library
- [ ] replace all inappropriate panic! macros with Result<T>
- [ ] replace all inappropriate assert! macros with debug_assert!
- [ ] maybe no need to save IBlock CHKs to bf
- [ ] strict dht store rules
- [x] refactor dht manager functions 
- [ ] refactor dht manager receive loop
- [ ] Use RwLock instead of Mutex when appropriate