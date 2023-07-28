import { myHtml } from "./utils";

describe("Plugin", () => {
	it("works", () => {
		expect(<div />).toBe("<div />");
	});
	it("collapses deeply nested statics", () => {
		expect(<div {...{ foo: "foo", ...{ bar: "bar", ...{ baz: true } } }} />).toBe('<div foo="foo" bar="bar" baz />');
	});
	it("correctly escapes non-static values", () => {
		const [foo, bar, baz] = [null, undefined, 123];
		expect(<div bar="123" foo={foo} {...{ foo, bar, baz }} />).toBe(
			myHtml`<div bar="123" ${{ $$spread: { foo, bar, baz } }} ${{ foo }} />`,
		);
	});
});
