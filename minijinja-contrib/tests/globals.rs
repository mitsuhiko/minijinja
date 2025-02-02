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
    Felis platea netus nisl sociosqu ssociis at morbi ante lobortis ssociis mi cubilia sociosqu ssociis. Nunc etiam posuere interdum sagittis dapibus nibh ipsum neque elementum magna scelerisque diam dictum arcu ssociis sociosqu nunc. Scelerisque ad sociosqu convallis leo facilisi felis in et id habitant orci consequat nisl mi. Porttitor dictumst hendrerit egestas eleifend ssociis lacus pellentesque nonummy eu ssociis facilisis justo ssociis vsociis felis odio. Lacinia sociosqu litora lectus pede elit curae dolor adipiscing quam sociosqu hac eros amet a enim. Inceptos fermentum ornare dis.

    Enim sociosqu praesent ipsum cubilia sociosqu commodo aliquet montes pellentesque sociosqu dapibus interdum. Elementum enim natoque quisque inceptos pede sociosqu sem sociosqu consectetuer lectus est. Sociis nisl sit hendrerit molestie parturient magna id orci erat proin phasellus ante sociosqu ssociis egestas. Posuere fames iaculis libero fermentum neque integer euismod fusce et euismod eu ac. Amet pellentesque per nunc sociis eleifend massa fames fermentum purus non. Ssociis feugiat dignissim nostra dis commodo sociosqu gravida nullam quisque sit et malesuada ante sociosqu. Lectus etiam justo phasellus proin ssoincidusociis sociosqu facilisi nec s placerat netus ssociis aptent. Sociosqu sapien.

    Magnis ssociis per commodo curabitur sociosqu platea condimentum enim sociosqu nullam litora proin molestie fusce. Molestie sociosqu ssociis lectus ligula.

    Sapien faucibus senectus convallis augue fames habitant morbi montes potenti nisi pretium mauris. Ante ssociis purus neque bibendum faucibus neque aliquam amet ssociis congue maecenas dolor dignissim. Habitasse nostra phasellus imperdiet id porta litora blandit in sed lacinia. Quam ssoincidusociis sociosqu massa proin dolor montes imperdiet cum sociosqu bibendum auctor.

    Cras natoque felis nostra ssociis arcu sociosqu scelerisque ssociis eros placerat proin rhoncus sociis est ssociis dapibus condimentum sed. Quisque et class placerat pharetra malesuada enim potenti fermentum natoque dolor risus auctor sociosqu nec risus ssociis sociosqu. Et bibendum egestas gravida pellentesque montes dapibus s donec hendrerit mollis et sit nibh amet. Cursus pretium molestie ssociis ridiculus convallis sociosqu sociis sagittis fermentum gravida quisque nostra. Ante scelerisque sociosqu non magnis ssociis lacinia feugiat risus erat risus.
    "###);

    assert_snapshot!(render!(in env, r"{% set RAND_SEED = 42 %}{{ lipsum(2, html=true) }}"), @r###"
    <p>Felis platea netus nisl sociosqu ssociis at morbi ante lobortis ssociis mi cubilia sociosqu ssociis. Nunc etiam posuere interdum sagittis dapibus nibh ipsum neque elementum magna scelerisque diam dictum arcu ssociis sociosqu nunc. Scelerisque ad sociosqu convallis leo facilisi felis in et id habitant orci consequat nisl mi. Porttitor dictumst hendrerit egestas eleifend ssociis lacus pellentesque nonummy eu ssociis facilisis justo ssociis vsociis felis odio. Lacinia sociosqu litora lectus pede elit curae dolor adipiscing quam sociosqu hac eros amet a enim. Inceptos fermentum ornare dis.</p>

    <p>Enim sociosqu praesent ipsum cubilia sociosqu commodo aliquet montes pellentesque sociosqu dapibus interdum. Elementum enim natoque quisque inceptos pede sociosqu sem sociosqu consectetuer lectus est. Sociis nisl sit hendrerit molestie parturient magna id orci erat proin phasellus ante sociosqu ssociis egestas. Posuere fames iaculis libero fermentum neque integer euismod fusce et euismod eu ac. Amet pellentesque per nunc sociis eleifend massa fames fermentum purus non. Ssociis feugiat dignissim nostra dis commodo sociosqu gravida nullam quisque sit et malesuada ante sociosqu. Lectus etiam justo phasellus proin ssoincidusociis sociosqu facilisi nec s placerat netus ssociis aptent. Sociosqu sapien.</p>
    "###);
}

#[test]
#[cfg(feature = "rand")]
fn test_randrange() {
    use minijinja_contrib::globals::randrange;

    let mut env = Environment::new();
    env.add_function("randrange", randrange);

    assert_snapshot!(render!(in env, r"{% set RAND_SEED = 42 %}{{ randrange(10) }}"), @"8");
    assert_snapshot!(render!(in env, r"{% set RAND_SEED = 42 %}{{ randrange(-50, 50) }}"), @"31");
}
