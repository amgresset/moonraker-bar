import { useEffect, useState } from "react";
import { backend } from "./backend";

// Thumbnails never change for a given file, so they are cached for the app's lifetime.
const cache = new Map<string, string | null>();
const inflight = new Map<string, Promise<string | null>>();

/** The slicer thumbnail of the current file, fetched once through the backend (keeps the API key there). */
export function useThumbnail(printerId: string | undefined, path: string | null | undefined): string | null {
  const key = printerId && path ? `${printerId}|${path}` : null;
  const [url, setUrl] = useState<string | null>(key ? (cache.get(key) ?? null) : null);

  useEffect(() => {
    if (!key || !printerId || !path) {
      setUrl(null);
      return;
    }
    if (cache.has(key)) {
      setUrl(cache.get(key) ?? null);
      return;
    }
    let alive = true;
    let p = inflight.get(key);
    if (!p) {
      p = backend
        .getThumbnail(printerId, path)
        .catch(() => null)
        .then((u) => {
          cache.set(key, u);
          inflight.delete(key);
          return u;
        });
      inflight.set(key, p);
    }
    p.then((u) => alive && setUrl(u));
    return () => {
      alive = false;
    };
  }, [key, printerId, path]);

  return url;
}
