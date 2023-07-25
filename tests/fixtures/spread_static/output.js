String.raw`<div foo="foo" cool ${{
    $$spread: {
        "bar:bar": `bar${true}`,
        baz
    }
}} />`
