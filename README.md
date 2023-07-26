# swc-plugin-static-jsx
SWC plugin to transform JSX calls to static templates

## Usage

```js
// In .swcrc:
{
  jsc: {
    experimental: {
      plugins: [
        ['swc-plugin-static-jsx', {
          // All config values are optional.
          template: 'String.raw',
          spread: '$$spread',
          child: '$$child',
          children: '$$children',
        }]
      ]
    }
  }
}
```

## Sample

```tsx
// input
<div foo="bar" baz={true} {...spread} {...{"std::string": "value"}}>
  The quick brown fox jumps over the <strong>lazy</strong> dog.
  {"<script>alert(\"You've been pwned!\")</script>"}
  {...children}
</div>

// output (approximate)
html`
<div foo="bar" baz ${{$$spread: spread}} std::string="value">
  The quick brown fox jumps over the<strong>lazy</strong>dog.
  ${{$$child: "<script>alert(\"You've been pwned!\")</script>"}}
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
