# Contributing to MiniJinja

Thanks for your interest in contributing to MiniJinja! MiniJinja welcomes
contribution from everyone in the form of suggestions, bug reports, pull
requests, and feedback. This document gives some guidance if you are thinking of
helping out.

## Submitting Bug Reports and Feature Requests

When reporting a bug or asking for help, please include enough details so that
others helping you can reproduce the behavior you are seeing.

Opening an issue is as easy.
Just [follow this link](https://github.com/mitsuhiko/minijinja/issues/new/choose)
and fill out the fields in the appropriate provided template.

When making a feature request, please make it clear what problem you intend to
solve with the feature and maybe provide some ideas for how to go about that.

## Running the Tests

When you want to contribute directly please make sure to run the tests and
format the code before making a pull request. Tests are also run in CI, but
it's typically easier to run them locally.

To run all tests a makefile is provided

```sh
make test
```

MiniJinja tests use the [Insta](https://insta.rs) testing framework. While not
required, it is recommended to use
the [`cargo insta review`](https://insta.rs/docs/cli/#review) command to review
and verify changes to the test results.

## Formatting the Code

If you want to format the code you can quickly run this command:

```sh
make format
```

## Conduct

This issue tracker follows the [Rust Code of Conduct]. For escalation or
moderation issues please contact Armin (armin.ronacher@active-4.com) instead of
the Rust moderation team.

[rust code of conduct]: https://www.rust-lang.org/policies/code-of-conduct