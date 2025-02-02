use insta::assert_snapshot;
use minijinja::{render, Environment};
use minijinja_contrib::globals::{cycler, joiner};

#[test]
fn test_cycler() {
    let mut env = Environment::new();
    env.add_function("cycler", cycler);

    assert_snapshot!(render!(in env, r"{% set c = cycler([1, 2]) -%}
next(): {{ c.next() }}
next(): {{ c.next() }}
next(): {{ c.next() }}
cycler: {{ c }}"), @r###"
    next(): 1
    next(): 2
    next(): 1
    cycler: Cycler { items: [1, 2], pos: 1 }
    "###);
}

#[test]
fn test_joiner() {
    let mut env = Environment::new();
    env.add_function("joiner", joiner);

    assert_snapshot!(render!(in env, r"{% set j = joiner() -%}
first: [{{ j() }}]
second: [{{ j() }}]
joiner: {{ j }}"), @r###"
    first: []
    second: [, ]
    joiner: Joiner { sep: ", ", used: true }
    "###);

    assert_snapshot!(render!(in env, r"{% set j = joiner('|') -%}
first: [{{ j() }}]
second: [{{ j() }}]
joiner: {{ j }}"), @r###"
    first: []
    second: [|]
    joiner: Joiner { sep: "|", used: true }
    "###);
}

#[test]
#[cfg(feature = "rand")]
#[cfg(target_pointer_width = "64")]
fn test_lispum() {
    // The small rng is pointer size specific.  Test on 64bit platforms only
    use minijinja_contrib::globals::lipsum;

    let mut env = Environment::new();
    env.add_function("lipsum", lipsum);

    assert_snapshot!(render!(in env, r"{% set RAND_SEED = 42 %}{{ lipsum(5) }}"), @r###"
    A auctor sociosqu orci laoreet ssociis convallis curae laoreet lacus dictum leo auctor sagittis. Accumsan etiam enim accumsan erat aliquet.

    Natoque orci nulla facilisi fringilla nisl integer sociosqu malesuada rhoncus nostra sociosqu. Orci nonummy convallis sociosqu inceptos quis s potenti rutrum hendrerit nostra nonummy luctus nullam. Praesent platea adipiscing interdum sagittis egestas nisl neque ssociis est scelerisque magnis nibh hac lacus. Hac dapibus lobortis s accumsan.

    Curae pretium conubia duis lectus condimentum massa aliquet sociosqu ipsum sociosqu mus ad aliquam placerat nunc. Dignissim egestas quis augue integer morbi ac ssociis diam curabitur fermentum lorem metus sociosqu diam id. Ssociis magnis diam ssociis natoque arcu ssociis nec blandit ssociis nec magnis sagittis sociosqu. Felis ssociis lectus curae iaculis praesent ssociis at vulputate eget dictumst feugiat accumsan nibh. Quam conubia condimentum donec eros conubia habitant penatibus dapibus ligula semper proin mi semper. Imperdiet sed metus ad risus sed donec convallis sociosqu eleifend feugiat ssociis sapien lacus diam. Fames quam est etiam sagittis.

    Vsociis pulvinar sagittis sociis mattis auctor egestas duis non justo sed bibendum. Commodo scelerisque nascetur imperdiet nulla ssociis porttitor porta pharetra parturient dolor dis rhoncus penatibus. Facilisis diam convallis neque commodo ipsum consequat quisque cursus quis elit nulla. Adipiscing penatibus ridiculus cras sociosqu.

    Nibh sed semper etiam et sem pede pharetra curae sociosqu aenean egestas consectetuer. Rutrum mauris lacus nisl hymenaeos eget fusce nonummy hac elementum fringilla sociosqu. Porta mollis nunc donec odio sociosqu luctus lobortis gravida neque enim mollis natoque hendrerit enim. Class potenti curabitur in curae cursus praesent erat ssoincidusociis sociosqu arcu. Bibendum eget eleifend neque gravida adipiscing.
    "###);

    assert_snapshot!(render!(in env, r"{% set RAND_SEED = 42 %}{{ lipsum(2, html=true) }}"), @r###"
    <p>A auctor sociosqu orci laoreet ssociis convallis curae laoreet lacus dictum leo auctor sagittis. Accumsan etiam enim accumsan erat aliquet.</p>

    <p>Natoque orci nulla facilisi fringilla nisl integer sociosqu malesuada rhoncus nostra sociosqu. Orci nonummy convallis sociosqu inceptos quis s potenti rutrum hendrerit nostra nonummy luctus nullam. Praesent platea adipiscing interdum sagittis egestas nisl neque ssociis est scelerisque magnis nibh hac lacus. Hac dapibus lobortis s accumsan.</p>
    "###);
}

#[test]
#[cfg(feature = "rand")]
fn test_randrange() {
    use minijinja_contrib::globals::randrange;

    let mut env = Environment::new();
    env.add_function("randrange", randrange);

    assert_snapshot!(render!(in env, r"{% set RAND_SEED = 42 %}{{ randrange(10) }}"), @"0");
    assert_snapshot!(render!(in env, r"{% set RAND_SEED = 42 %}{{ randrange(-50, 50) }}"), @"-50");
}
