import { useEffect, useState, type ChangeEvent, type FormEvent } from "react";
import { Plus, Trash2, Pencil, Wifi, Check, Loader2 } from "lucide-react";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import { useStore } from "@/store";
import { backend } from "@/lib/backend";
import type { PrinterConfig, PrinterInput } from "@/lib/types";
import { cn } from "@/lib/format";
import { Button, Card, Field, IconButton, Input } from "./ui";

export function Settings() {
  const { printers, snapshots } = useStore();
  const [editing, setEditing] = useState<PrinterConfig | "new" | null>(printers.length === 0 ? "new" : null);

  return (
    <div className="flex flex-col gap-3">
      <section>
        <div className="mb-1.5 flex items-center justify-between">
          <h2 className="text-[11px] font-medium uppercase tracking-wide text-fg-2">Imprimantes</h2>
          {editing === null && (
            <Button size="sm" variant="ghost" icon={<Plus size={13} />} onClick={() => setEditing("new")}>
              Ajouter
            </Button>
          )}
        </div>

        {editing !== null ? (
          <PrinterForm
            initial={editing === "new" ? null : editing}
            onDone={() => setEditing(null)}
          />
        ) : printers.length === 0 ? (
          <Card className="py-6 text-center text-[12px] text-fg-3">Aucune imprimante.</Card>
        ) : (
          <Card className="divide-y divide-line p-0">
            {printers.map((p) => {
              const s = snapshots[p.id];
              const online = s?.connection === "connected";
              return (
                <div key={p.id} className="flex items-center gap-2.5 px-3 py-2">
                  <span className={cn("h-2 w-2 shrink-0 rounded-full", online ? "bg-ok" : "bg-fg-3")} />
                  <div className="min-w-0 flex-1">
                    <div className="truncate font-medium">{p.name}</div>
                    <div className="truncate text-[11px] text-fg-3">
                      {p.url}
                      {s?.hostname ? ` · ${s.hostname}` : ""}
                    </div>
                  </div>
                  <IconButton title="Modifier" onClick={() => setEditing(p)}>
                    <Pencil size={13} />
                  </IconButton>
                  <DeleteButton id={p.id} />
                </div>
              );
            })}
          </Card>
        )}
      </section>

      <section>
        <h2 className="mb-1.5 text-[11px] font-medium uppercase tracking-wide text-fg-2">Général</h2>
        <Card className="p-0">
          <AutostartRow />
        </Card>
      </section>
    </div>
  );
}

function DeleteButton({ id }: { id: string }) {
  const [armed, setArmed] = useState(false);
  useEffect(() => {
    if (!armed) return;
    const t = setTimeout(() => setArmed(false), 3000);
    return () => clearTimeout(t);
  }, [armed]);
  return (
    <IconButton
      title={armed ? "Confirmer la suppression" : "Supprimer"}
      className={cn(armed && "bg-danger/15 text-danger hover:bg-danger/25 hover:text-danger")}
      onClick={async () => {
        if (!armed) return setArmed(true);
        try {
          await backend.removePrinter(id);
        } catch (e) {
          useStore.getState().setError(String(e));
        }
      }}
    >
      {armed ? <Check size={13} /> : <Trash2 size={13} />}
    </IconButton>
  );
}

function PrinterForm({ initial, onDone }: { initial: PrinterConfig | null; onDone: () => void }) {
  const [form, setForm] = useState<PrinterInput>({
    name: initial?.name ?? "",
    url: initial?.url ?? "http://",
    api_key: initial?.api_key ?? "",
    camera_url: initial?.camera_url ?? "",
    web_url: initial?.web_url ?? "",
  });
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; text: string } | null>(null);
  const [saving, setSaving] = useState(false);

  const set = (k: keyof PrinterInput) => (e: ChangeEvent<HTMLInputElement>) => {
    setForm((f) => ({ ...f, [k]: e.target.value }));
    setTestResult(null);
  };

  const test = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      const text = await backend.testConnection(form);
      setTestResult({ ok: true, text });
    } catch (e) {
      setTestResult({ ok: false, text: String(e) });
    } finally {
      setTesting(false);
    }
  };

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    try {
      if (initial) await backend.updatePrinter(initial.id, form);
      else await backend.addPrinter(form);
      onDone();
    } catch (err) {
      useStore.getState().setError(String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Card>
      <form onSubmit={submit} className="flex flex-col gap-2.5">
        <Field label="Nom">
          <Input value={form.name} onChange={set("name")} placeholder="Voron 2.4" autoFocus />
        </Field>
        <Field label="URL Moonraker" hint="http://imprimante.local:7125 — ou l'URL de Mainsail/Fluidd si Moonraker est derrière nginx.">
          <Input value={form.url} onChange={set("url")} placeholder="http://voron.local:7125" required spellCheck={false} />
        </Field>
        <Field label="Clé API" hint="Facultative sauf si l'authentification est activée dans Moonraker.">
          <Input value={form.api_key ?? ""} onChange={set("api_key")} placeholder="—" spellCheck={false} />
        </Field>
        <Field label="Interface web" hint="Mainsail ou Fluidd. Vide = même hôte que Moonraker sans le port.">
          <Input value={form.web_url ?? ""} onChange={set("web_url")} placeholder="http://voron.local" spellCheck={false} />
        </Field>
        <Field label="Snapshot caméra" hint="Absolue, ou relative à l'URL Moonraker, ex. /webcam/?action=snapshot">
          <Input value={form.camera_url ?? ""} onChange={set("camera_url")} placeholder="/webcam/?action=snapshot" spellCheck={false} />
        </Field>

        {testResult && (
          <div className={cn("rounded-lg px-2.5 py-1.5 text-[12px]", testResult.ok ? "bg-ok/10 text-ok" : "bg-danger/10 text-danger")}>
            {testResult.text}
          </div>
        )}

        <div className="flex items-center gap-2 pt-1">
          <Button type="button" size="sm" variant="ghost" icon={testing ? <Loader2 size={13} className="animate-spin" /> : <Wifi size={13} />} disabled={testing} onClick={test}>
            Tester
          </Button>
          <div className="flex-1" />
          <Button type="button" size="sm" variant="ghost" onClick={onDone}>
            Annuler
          </Button>
          <Button type="submit" size="sm" variant="primary" disabled={saving}>
            {initial ? "Enregistrer" : "Ajouter"}
          </Button>
        </div>
      </form>
    </Card>
  );
}

function AutostartRow() {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  useEffect(() => {
    isEnabled().then(setEnabled).catch(() => setEnabled(false));
  }, []);
  const toggle = async () => {
    if (enabled == null) return;
    try {
      if (enabled) await disable();
      else await enable();
      setEnabled(!enabled);
    } catch (e) {
      useStore.getState().setError(String(e));
    }
  };
  return (
    <button type="button" onClick={toggle} className="flex w-full items-center justify-between px-3 py-2 text-left hover:bg-surface-3/60">
      <div>
        <div className="font-medium">Lancer au démarrage</div>
        <div className="text-[11px] text-fg-3">Démarre discrètement dans la barre de menus.</div>
      </div>
      <Toggle on={enabled === true} />
    </button>
  );
}

function Toggle({ on }: { on: boolean }) {
  return (
    <span className={cn("relative inline-block h-5 w-8 rounded-full transition", on ? "bg-accent" : "bg-fg/20")}>
      <span className={cn("absolute top-0.5 left-0.5 h-4 w-4 rounded-full bg-white shadow transition", on && "translate-x-3")} />
    </span>
  );
}
