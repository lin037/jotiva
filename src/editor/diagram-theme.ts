// One entry per semantic role. Visible diagrams normally share a palette.
const resolvedColors = new Map<string, { value: string; context: string; hex: string }>();

export function diagramTheme(host: HTMLElement) {
  const style = getComputedStyle(host);
  const darkMode = host.ownerDocument.documentElement.dataset.jotScheme === "dark";
  const colorContext = [darkMode, style.colorScheme, style.color].join("|");
  let sample: HTMLCanvasElement | undefined;
  function color(role: string) {
    const value = style.getPropertyValue("--jot-color-" + role).trim();
    if (/^#(?:[\da-f]{3,4}|[\da-f]{6}|[\da-f]{8})$/i.test(value)) return value;
    const cached = resolvedColors.get(role);
    if (cached && cached.value === value && cached.context === colorContext) return cached.hex;
    // Mermaid computes shades in sRGB. Resolve CSS color-mix(), light-dark()
    // and named colors in the host's color context, outside the editing surface.
    if (!sample) {
      sample = host.ownerDocument.createElement("canvas");
      sample.width = sample.height = 1;
      sample.hidden = true;
      sample.style.colorScheme = style.colorScheme;
      sample.style.color = style.color;
      host.ownerDocument.body.append(sample);
    }
    sample.style.backgroundColor = "";
    sample.style.backgroundColor = value;
    const context = sample.getContext("2d", { willReadFrequently: true })!;
    context.globalCompositeOperation = "copy";
    context.fillStyle = getComputedStyle(sample).backgroundColor;
    context.fillRect(0, 0, 1, 1);
    const rgba = context.getImageData(0, 0, 1, 1).data;
    const bytes = rgba[3] === 255 ? rgba.subarray(0, 3) : rgba;
    const hex = "#" + Array.from(bytes, byte => byte.toString(16).padStart(2, "0")).join("");
    resolvedColors.set(role, { value, context: colorContext, hex });
    return hex;
  }
  try {
    const paper = color("bg"), surface = color("surface"), text = color("text");
    const muted = color("muted"), accent = color("accent"), line = color("line-strong");
    const quote = color("quote"), error = color("error");
    const fill = color("accent-fill");
    // Mermaid names the same categories differently in each diagram family.
    const series: Record<string, string> = {};
    for (let index = 0; index < 12; index++) {
      const swatch = color("series-" + (index + 1));
      series["pie" + (index + 1)] = swatch;
      series["cScale" + index] = swatch;
      series["cScaleInv" + index] = muted;
      if (index < 8) series["git" + index] = swatch;
    }
    return {
      ...series,
      darkMode,
      fontFamily: style.fontFamily, fontSize: style.fontSize,
      background: paper, primaryColor: surface, primaryTextColor: text, primaryBorderColor: accent,
      secondaryColor: quote, secondaryTextColor: text, tertiaryColor: paper, tertiaryTextColor: text,
      lineColor: muted, arrowheadColor: muted, clusterBkg: quote, clusterBorder: line,
      edgeLabelBackground: paper, titleColor: text,
      noteBkgColor: quote, noteBorderColor: line, noteTextColor: text,
      attributeBackgroundColorOdd: surface, attributeBackgroundColorEven: paper,
      branchLabelColor: text, scaleLabelColor: text,
      pieStrokeColor: paper, pieOuterStrokeColor: line, pieOpacity: 1,
      sectionBkgColor: surface, sectionBkgColor2: quote, altSectionBkgColor: paper,
      gridColor: line, todayLineColor: accent, vertLineColor: accent, excludeBkgColor: surface,
      taskBkgColor: fill, taskBorderColor: accent, taskTextColor: color("on-accent"),
      taskTextOutsideColor: text, taskTextDarkColor: text, taskTextClickableColor: accent,
      activeTaskBkgColor: color("active"), activeTaskBorderColor: accent,
      doneTaskBkgColor: surface, doneTaskBorderColor: line, critBkgColor: fill, critBorderColor: error,
    };
  } finally {
    sample?.remove();
  }
}
