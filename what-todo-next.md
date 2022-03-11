# what to do next
make ping test works
and write dht op tests with it

# BUGS TODO
- [] when testing with cocoon virtual sometimes find value message.key.len() ==0 somehow
# TODO
- [] resumeable task for encoding, downloading
- [] use tower library
- [] replace all inappropriate panic! macros with Result<T>
- [] replace all inappropriate assert! macros with debug_assert!
- [] maybe no need to save IBlock CHKs to bf