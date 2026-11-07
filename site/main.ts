// Points the download buttons at the newest release's nodesk.exe, with its version and size.
// Without JS, or before any release exists, the buttons keep their static fallback links.

const REPO = "seifsg/nodesk";
const ASSET = "nodesk.exe";

interface Asset {
  name: string;
  size: number;
  browser_download_url: string;
}

interface Release {
  tag_name: string;
  assets: Asset[];
}

/** True when the API answer has the shape we read. */
function isRelease(x: unknown): x is Release {
  const r = x as Partial<Release> | null;
  return typeof r?.tag_name === "string" && Array.isArray(r.assets);
}

/** "266 KB" style size. */
function kb(bytes: number): string {
  return `${Math.round(bytes / 1024)} KB`;
}

const RELEASES = `https://github.com/${REPO}/releases/`;

/** Latest release's exe; "none" when no release exists yet (404), "unknown" when the API fails (rate limit, offline). */
async function latestExe(): Promise<{ tag: string; asset: Asset } | "none" | "unknown"> {
  try {
    const res = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
      headers: { Accept: "application/vnd.github+json" },
    });
    if (res.status === 404) return "none";
    if (!res.ok) return "unknown";
    const body: unknown = await res.json();
    if (!isRelease(body)) return "unknown";
    // Only ever link to this repo's own release files.
    const asset = body.assets.find((a) => a.name === ASSET && a.browser_download_url.startsWith(RELEASES));
    return asset ? { tag: body.tag_name, asset } : "none";
  } catch {
    return "unknown";
  }
}

/** Fills every [data-download] link and [data-download-note] label. */
async function wireDownloads(): Promise<void> {
  const links = document.querySelectorAll<HTMLAnchorElement>("[data-download]");
  const notes = document.querySelectorAll<HTMLElement>("[data-download-note]");
  const found = await latestExe();
  for (const a of links) {
    if (typeof found === "object") {
      a.href = found.asset.browser_download_url;
      a.textContent = `Download ${found.tag}`;
    } else if (found === "none") {
      a.href = `https://github.com/${REPO}#install`;
      a.textContent = "Get it on GitHub";
    }
  }
  const note =
    typeof found === "object"
      ? `nodesk.exe · ${kb(found.asset.size)} · Windows 10 and 11, x64 · tested on 11`
      : found === "none"
        ? "First release coming soon. Build it from source today."
        : "Windows 10 and 11, x64 · tested on 11 · free and open source";
  notes.forEach((n) => (n.textContent = note));
}

void wireDownloads();
