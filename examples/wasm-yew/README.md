# wasm-basic

A simple example of how to use the `yew` and MiniJinja together to render a
template from a string.

This example uses [Trunk](https://trunkrs.dev/), which will build the project
and bundle everything together. It's similar to wasm-pack, but wasm-pack doesn't
work very well with recent OpenSSL versions.

Once you have trunk installed (`cargo install trunk`), you can run the example:

```sh
$ trunk serve
```

This will run and build the project locally on http://localhost:8080.