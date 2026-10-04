# autoreload

An example that shows how auto reloading works with MiniJinja.  While the
example is running you can edit the templates in the [templates](templates)
folder to see them change in real time.

Templates loaded through `path_loader` are checked for changes whenever they
are looked up and are reloaded individually.

```console
$ cargo run
```

To disable auto reloading:

```console
$ DISABLE_AUTORELOAD=1 cargo run
```
