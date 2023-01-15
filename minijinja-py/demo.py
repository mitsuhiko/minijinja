from minijinja_py import Environment

class Wat(object):
    def __init__(self):
        self.a = 42
        self.b = 23
    def __repr__(self):
        return f"<Wat a={self.a} b={self.b}>"


env = Environment({
    "index.txt": "Hello {{ name }}!\n{{ seq }}\n{{ wat }}\n{{ wat.a }}"
})
print(env.render_template(
    "index.txt",
    name="John",
    seq=[True, False, None, [1, 2, 3], {"foo": "bar"}],
    wat=Wat()
))
