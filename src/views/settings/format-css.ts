import { parse } from "postcss";

export function formatCss(css: string) {
  const root = parse(css);
  const depths = new WeakMap<object, number>();
  root.raws.after = "";
  root.raws.semicolon = true;
  root.walk(node => {
    const depth = node.parent?.type === "root" ? 0 : (depths.get(node.parent!) ?? 0) + 1;
    if (depth > 64) throw new Error();
    depths.set(node, depth);
    const indent = "  ".repeat(depth);
    if (!node.raws.before?.trim()) {
      node.raws.before = node === root.first ? "" : "\n" + indent;
    }
    if (node.type === "decl" && /^\s*:\s*$/.test(node.raws.between ?? ":")) {
      node.raws.between = node.value ? ": " : node.raws.between;
    }
    if (node.type === "rule" || (node.type === "atrule" && node.nodes)) {
      if (!node.raws.between?.trim()) node.raws.between = " ";
      if (!node.raws.after?.trim()) node.raws.after = node.nodes?.length ? "\n" + indent : "";
      node.raws.semicolon = true;
    }
  });
  return root.nodes.length ? root.toString() + "\n" : "";
}
