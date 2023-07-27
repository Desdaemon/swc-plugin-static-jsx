# swc-plugin-static-jsx
[![Rust](https://github.com/Desdaemon/swc-plugin-static-jsx/actions/workflows/rust.yml/badge.svg)](https://github.com/Desdaemon/swc-plugin-static-jsx/actions/workflows/rust.yml)
[![Node.js E2E](https://github.com/Desdaemon/swc-plugin-static-jsx/actions/workflows/e2e.yml/badge.svg)](https://github.com/Desdaemon/swc-plugin-static-jsx/actions/workflows/e2e.yml)

SWC plugin to transform JSX calls to static templates

## Usage

```js
// In .swcrc:
{
  jsc: {
    experimental: {
      plugins: [
        // All config values are optional.
        ['swc-plugin-static-jsx', {
          // If an identifier is supplied, it should not be an ambient global.
          template: 'String.raw',
          // If supplied, template will be imported as `import { template } from 'my-library'`
          importSource: 'my-library'
          spread: '$$spread',
          child: '$$child',
          children: '$$children',
        }]
      ]
    }
  }
}
```

In your tsconfig.json, `compilerOptions.jsx` should be set to 'preserve'. You will also need to
provide your own JSX-related types under `namespace JSX`.

## Sample

```tsx
let unsanitized = "<script>alert(\"You've been pwned!\")</script>"
// input
<div foo="bar" baz={true} {...spread} {...{"std::string": "value"}}>
  The quick brown fox jumps over the <strong>lazy</strong> dog.
  {unsanitized}
  {...children}
</div>

// output (approximate)
html`
<div foo="bar" baz ${{$$spread: spread}} std::string="value">
  The quick brown fox jumps over the<strong>lazy</strong>dog.
  ${{$$child: unsanitized}}
  ${{$$children: children}}
</div>`
```

Sample implementation of `html`:
```ts
function html(raw, ...children: Record<string, unknown>[]) {
  all: for (const child of children) {
    for (const key in child) {
      switch (key) {
      case "$$child":
        // ..
        continue all
      case "$$children":
        // ..
        continue all
      case "$$spread":
        // ..
        continue all
      default:
        // ..
      }
    }
  }
}
```
