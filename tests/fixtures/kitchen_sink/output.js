String.raw`<div foo="bar" baz ${{
    $$spread: spread
}} std::string="value">The quick brown fox jumps over the<strong>lazy</strong>dog.${{
    $$child: "<script>alert(\"You've been pwned!\")</script>"
}} ${{
    $$children: children
}} </div>`;
