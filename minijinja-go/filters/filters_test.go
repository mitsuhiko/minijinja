package filters

import (
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
