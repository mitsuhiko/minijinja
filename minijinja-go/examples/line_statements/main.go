// Example: line-statements
//
// This example demonstrates using line statements and line comments.
// Line statements are an alternative syntax where blocks can be placed
// on their own line with a prefix character.
package main

import (
	"fmt"
	"log"

	"github.com/mitsuhiko/minijinja/minijinja-go/v2"
	"github.com/mitsuhiko/minijinja/minijinja-go/v2/syntax"
)

const helloTemplate = `## this is a line comment
<ul>
  # for item in seq
    ## again another comment here.  Removed entirely
    <li>{{ item }}
  # endfor
</ul>
`

func main() {
	env := minijinja.NewEnvironment()

	// Configure line statement and comment prefixes
	env.SetSyntax(syntax.SyntaxConfig{
		BlockStart:          "{%",
		BlockEnd:            "%}",
		VarStart:            "{{",
		VarEnd:              "}}",
		CommentStart:        "{#",
		CommentEnd:          "#}",
		LineStatementPrefix: "#",
		LineCommentPrefix:   "##",
	})

	err := env.AddTemplate("hello.txt", helloTemplate)
	if err != nil {
		log.Fatal(err)
	}

	tmpl, err := env.GetTemplate("hello.txt")
	if err != nil {
		log.Fatal(err)
	}

	result, err := tmpl.Render(map[string]any{
		"seq": []string{"foo", "bar"},
	})
	if err != nil {
		log.Fatal(err)
	}

	fmt.Println(result)
}
