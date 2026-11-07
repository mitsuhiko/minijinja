package filters

import (
	"strings"
	"testing"

	"github.com/mitsuhiko/minijinja/minijinja-go/v3/value"
)

type comparableNum struct{ n int }

func (c *comparableNum) GetAttr(string) value.Value { return value.Undefined() }

func (c *comparableNum) ObjectCmp(other value.Object) (int, bool) {
	o, ok := other.(*comparableNum)
	if !ok {
		return 0, false
	}
	// Comparators can return any negative or positive value, not just -1 or 1.
	return c.n - o.n, true
}

func TestMinMaxCustomComparison(t *testing.T) {
	for _, tc := range []struct {
		name  string
		items []int
		want  int
	}{
		{"min", []int{10, 2, 7}, 2},
		{"max", []int{2, 10, 7}, 10},
	} {
		for _, attr := range []string{"", "key"} {
			t.Run(tc.name+"/"+attr, func(t *testing.T) {
				items := make([]value.Value, len(tc.items))
				var kwargs map[string]value.Value
				if attr != "" {
					kwargs = map[string]value.Value{"attribute": value.FromString(attr)}
				}
				for i, n := range tc.items {
					items[i] = value.FromObject(&comparableNum{n})
					if attr != "" {
						items[i] = value.FromMap(map[string]value.Value{attr: items[i]})
					}
				}
				filter := FilterMin
				if tc.name == "max" {
					filter = FilterMax
				}
				got, err := filter(nil, value.FromSlice(items), nil, kwargs)
				if err != nil {
					t.Fatal(err)
				}
				if attr != "" {
					got = got.GetAttr(attr)
				}
				obj, ok := got.AsObject()
				if !ok {
					t.Fatalf("expected a custom object, got %v", got)
				}
				if n := obj.(*comparableNum).n; n != tc.want {
					t.Fatalf("got %d, want %d", n, tc.want)
				}
			})
		}
	}
}

func TestMinMaxTiesReturnFirst(t *testing.T) {
	for _, tc := range []struct {
		name   string
		items  []value.Value
		kwargs map[string]value.Value
	}{
		{"bool-int", []value.Value{value.FromBool(true), value.FromInt(1)}, nil},
		{"int-float", []value.Value{value.FromInt(1), value.FromFloat(1.0)}, nil},
		{"case-insensitive", []value.Value{value.FromString("a"), value.FromString("A")}, nil},
		{"attribute", []value.Value{
			value.FromMap(map[string]value.Value{"k": value.FromInt(1), "n": value.FromString("x")}),
			value.FromMap(map[string]value.Value{"k": value.FromInt(1), "n": value.FromString("y")}),
		}, map[string]value.Value{"attribute": value.FromString("k")}},
	} {
		for name, filter := range map[string]func(State, value.Value, []value.Value, map[string]value.Value) (value.Value, error){
			"min": FilterMin,
			"max": FilterMax,
		} {
			t.Run(name+"/"+tc.name, func(t *testing.T) {
				got, err := filter(nil, value.FromSlice(tc.items), nil, tc.kwargs)
				if err != nil {
					t.Fatal(err)
				}
				if got.Repr() != tc.items[0].Repr() {
					t.Fatalf("got %s, want first item %s", got.Repr(), tc.items[0].Repr())
				}
			})
		}
	}
}

func TestGroupByTies(t *testing.T) {
	item := func(key value.Value, id int64) value.Value {
		return value.FromMap(map[string]value.Value{"key": key, "id": value.FromInt(id)})
	}
	render := func(groups value.Value) string {
		var b strings.Builder
		items, _ := groups.AsSlice()
		for _, group := range items {
			b.WriteString(group.GetAttr("grouper").Repr())
			b.WriteByte(':')
			list, _ := group.GetAttr("list").AsSlice()
			for _, item := range list {
				b.WriteByte(' ')
				b.WriteString(item.GetAttr("id").Repr())
			}
			b.WriteByte(';')
		}
		return b.String()
	}
	for _, tc := range []struct {
		name   string
		items  []value.Value
		kwargs map[string]value.Value
		want   string
	}{
		{"bool-int", []value.Value{item(value.FromBool(true), 1), item(value.FromInt(1), 2)}, nil, "True: 1 2;"},
		{"int-float", []value.Value{item(value.FromInt(1), 1), item(value.FromFloat(1.0), 2)}, nil, "1: 1 2;"},
		{"case-insensitive", []value.Value{
			item(value.FromString("b"), 1),
			item(value.FromString("x"), 2),
			item(value.FromString("B"), 3),
			item(value.FromString("X"), 4),
			item(value.FromString("x"), 5),
		}, nil, "'b': 1 3;'x': 2 4 5;"},
		{"case-sensitive", []value.Value{
			item(value.FromString("x"), 1),
			item(value.FromString("X"), 2),
			item(value.FromString("x"), 3),
		}, map[string]value.Value{"case_sensitive": value.FromBool(true)}, "'X': 2;'x': 1 3;"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			got, err := FilterGroupBy(nil, value.FromSlice(tc.items), []value.Value{value.FromString("key")}, tc.kwargs)
			if err != nil {
				t.Fatal(err)
			}
			if s := render(got); s != tc.want {
				t.Fatalf("got %s, want %s", s, tc.want)
			}
		})
	}
}

func TestCaseConversions(t *testing.T) {
	filters := map[string]func(State, value.Value, []value.Value, map[string]value.Value) (value.Value, error){
		"upper":      FilterUpper,
		"lower":      FilterLower,
		"capitalize": FilterCapitalize,
		"title":      FilterTitle,
	}
	for _, tc := range []struct {
		filter, input, want string
	}{
		{"title", "", ""},
		{"title", " -( ", " -( "},
		{"title", "hello wORLD", "Hello World"},
		{"title", "éCOLE straße", "École Straße"},
		{"title", "aİ", "Ai\u0307"},
		{"title", "ß ﬁre", "SS FIre"},
		{"title", "ΑΣ", "Ας"},
		{"title", "ΣΑΣΑ", "Σασα"},
		{"title", "ΟΔΟΣ\u0301 ΟΔΟΣA", "Οδος\u0301 Οδοσa"},
		{"capitalize", "", ""},
		{"capitalize", "ΑΣ", "Ας"},
		{"capitalize", "ß", "Ss"},
		{"capitalize", "Ǆ", "ǅ"},
		{"capitalize", "ﬃX", "Ffix"},
		{"capitalize", "ა", "ა"},
		{"capitalize", "ᾀ", "ᾈ"},
		{"capitalize", "ᾷ", "Α\u0342\u0345"},
		{"upper", "ß", "SS"},
		{"upper", "ᾳ", "ΑΙ"},
		{"lower", "İ", "i\u0307"},
		{"lower", "ΑΣ", "ας"},
		{"lower", "Σ", "σ"},
		{"lower", "ΟΔΟΣ\u0301 ΟΔΟΣA", "οδος\u0301 οδοσa"},
	} {
		got, err := filters[tc.filter](nil, value.FromString(tc.input), nil, nil)
		if err != nil {
			t.Fatal(err)
		}
		if s, _ := got.AsString(); s != tc.want {
			t.Errorf("%s(%q) = %q, want %q", tc.filter, tc.input, s, tc.want)
		}
	}
}
