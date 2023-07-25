String.raw`<div ${{
    $$spread: {
        "bar:bar": `bar${true}`,
        baz
    }
}} ${{
    "foo": "foo",
    "cool": true
}} />`
