export interface Config {
	/**
	 * The name of the template function.
	 * Should be a function with the signature of {@link TemplateFunction}, unless you customize
	 * some of the configs below.
	 *
	 * If null, untagged templates will be output.
	 * @default String.raw
	 */
	template?: string | null;
	/**
	 * Where to import the template function from.
	 * If undefined, the template function needs to be defined/imported within the file.
	 */
	importSource?: string;
	/**
	 * The name of the spread key to transform spread attributes.
	 * @default "$$spread"
	 */
	spread?: string;
	/**
	 * The name of the child key to transform interpolated children.
	 * @default "$$child"
	 */
	child?: string;
	/**
	 * The name of the children key to transform spread children.
	 * @default "$$children"
	 */
	children?: string;
}

/**
 * Should return a `JSX.Element`.
 */
export type TemplateFunction = (template: TemplateStringsArray, ...children: Child[]) => unknown;

export type Child = { $$children: unknown } | { $$child: unknown } | { $$spread: unknown } | Record<string, unknown>;

declare const module: WebAssembly.Module;
export default module;
