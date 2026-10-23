package main

import (
	"go/ast"
	"go/format"
	"go/parser"
	"go/token"
	"os"
)

func main() {
	files := token.NewFileSet()
	source, err := parser.ParseFile(files, os.Args[1], nil, 0)
	if err != nil {
		panic(err)
	}
	selected := map[string]bool{"Reply": true, "Choice": true, "Message": true, "Body": true}
	count := 0
	for _, declaration := range source.Decls {
		keep := false
		switch item := declaration.(type) {
		case *ast.GenDecl:
			if item.Tok == token.TYPE && len(item.Specs) == 1 {
				keep = selected[item.Specs[0].(*ast.TypeSpec).Name.Name]
			}
		case *ast.FuncDecl:
			if item.Recv != nil && len(item.Recv.List) == 1 {
				receiver := item.Recv.List[0].Type
				if pointer, ok := receiver.(*ast.StarExpr); ok {
					receiver = pointer.X
				}
				if identifier, ok := receiver.(*ast.Ident); ok {
					keep = identifier.Name == "Body"
				}
			}
		}
		if keep {
			if err := format.Node(os.Stdout, files, declaration); err != nil {
				panic(err)
			}
			if _, err := os.Stdout.WriteString("\n\n"); err != nil {
				panic(err)
			}
			count++
		}
	}
	if count != 6 {
		panic("expected four actual DTO declarations and two Body JSON methods")
	}
}
