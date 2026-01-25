// Example: recursive-for
//
// This example demonstrates recursive loops.
package main

import (
	"fmt"
	"log"

	minijinja "github.com/mitsuhiko/minijinja/minijinja-go/v2"
)

type Item struct {
	Link     string `json:"link"`
	Title    string `json:"title"`
	Children []Item `json:"children"`
}

func main() {
	env := minijinja.NewEnvironment()
	env.SetDebug(true)

	if err := env.AddTemplate("loop.html", `
    <ul class="nav">
    {% for item in nav recursive %}
      <li><a href={{ item.link }}>{{ item.title }}</a>{%
        if item.children %}<ul>{{ loop(item.children) }}</ul>{% endif %}</li>
    {% endfor %}
    </ul>
    `); err != nil {
		log.Fatal(err)
	}

	tmpl, err := env.GetTemplate("loop.html")
	if err != nil {
		log.Fatal(err)
	}

	result, err := tmpl.Render(map[string]any{
		"nav": []Item{
			{
				Link:  "/",
				Title: "Index",
			},
			{
				Link:  "/docs",
				Title: "Documentation",
				Children: []Item{
					{
						Link:  "/docs/installation",
						Title: "Installation",
					},
					{
						Link:  "/docs/faq",
						Title: "FAQ",
					},
				},
			},
		},
	})
	if err != nil {
		log.Fatal(err)
	}

	fmt.Println(result)
}
