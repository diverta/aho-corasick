### About

This is a fork of the popular [aho-corasick](https://github.com/BurntSushi/aho-corasick) crate which adds the implementation for Async processing using AhoCorasick.

Consider the Async code in beta - working, tested and already used in our internal projects. Due to [various reasons](https://github.com/BurntSushi/aho-corasick/issues/133), there is currently no intention of integrating these features into the main crate. However feel free to use this implementation if you need it. As it is actively used, improvements are expected - and improvements PRs are welcome!

### Usage  & async interface

`async` feature must be explicitly enabled in order to use async features. Then, 3 new public methods will become available to AhoCorasick struct :
- `async_reader` : Produces an AhoCorasickAsyncReader which implements AsyncRead trait, wrapping user-provided AsyncRead source, and yielding chunks with replaced bytes when polling from it
- `async_writer` : Produces an AhoCorasickAsyncWriter which implements AsyncWrite trait, wrapping user-provided AsyncWrite sink, and similarily writing to it will write replaced chunks to the original sink.
- `try_async_stream_replace_all` : A mostly standalone helper function which is somewhat trivial to implement using either `async_reader` or `async_writer`, however I though it would be convenient to have as an async alternative to the existing `try_stream_replace_all` method.

### Examples

- Using `async_reader`

```rust
use aho_corasick::AhoCorasick;
use futures::AsyncReadExt;

let patterns = &["fox", "brown", "quick"];
let replacements = &["bear", "white", "slow"];
let haystack = futures::io::Cursor::new("The quick brown fox.");

let ac = AhoCorasick::new(patterns).unwrap();
let mut ac_async_reader = ac.async_reader(haystack, replacements).unwrap();
let mut result = String::new();
futures::executor::block_on(async {
    ac_async_reader.read_to_string(&mut result).await.unwrap();
});
assert_eq!(&result, "The slow white bear.");
```

- Using `async_writer`

```rust
use aho_corasick::AhoCorasick;
use futures::{AsyncReadExt, AsyncWriteExt};

let patterns = &["fox", "brown", "quick"];
let replacements = &["bear", "white", "slow"];
let mut haystack = futures::io::Cursor::new("The quick brown fox.");

let ac = AhoCorasick::new(patterns).unwrap();
let mut result: futures::io::Cursor<Vec<u8>> = futures::io::Cursor::new(Vec::new());
let mut ac_async_writer = ac.async_writer(&mut result, replacements).unwrap();
futures::executor::block_on(async {
    let mut buf = [0u8; 10];
    loop {
        let bytes_read = haystack.read(&mut buf).await.unwrap();
        if bytes_read > 0 {
            ac_async_writer.write(&buf[..bytes_read]).await.unwrap();
        } else {
            ac_async_writer.close().await.unwrap();
            break;
        }
    }
});
assert_eq!(&String::from_utf8(result.get_ref().to_vec()).unwrap(), "The slow white bear.");
```
