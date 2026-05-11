import { useState } from "react"

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Slider } from "@/components/ui/slider"

export function PrimitiveShowcase() {
  const [value, setValue] = useState<number[]>([80])

  return (
    <main className="min-h-screen bg-bg p-8 text-fg">
      <div className="mx-auto flex max-w-[480px] flex-col gap-6">
        <header>
          <h1 className="text-lg font-semibold">shadcn primitive sandbox</h1>
          <p className="text-fg-secondary text-xs">
            Button · Dialog · Input · Select · Slider
          </p>
        </header>

        <section className="flex flex-col gap-2">
          <label className="text-fg-tertiary text-[10px] font-semibold uppercase tracking-[0.12em]">
            Button
          </label>
          <div className="flex gap-2">
            <Button variant="primary">Primary</Button>
            <Button variant="secondary">Secondary</Button>
            <Button variant="ghost">Ghost</Button>
            <Button variant="danger">Danger</Button>
          </div>
        </section>

        <section className="flex flex-col gap-2">
          <label className="text-fg-tertiary text-[10px] font-semibold uppercase tracking-[0.12em]">
            Input
          </label>
          <Input placeholder="Type something…" />
        </section>

        <section className="flex flex-col gap-2">
          <label className="text-fg-tertiary text-[10px] font-semibold uppercase tracking-[0.12em]">
            Select
          </label>
          <Select defaultValue="sonnet">
            <SelectTrigger>
              <SelectValue placeholder="Pick a model" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="opus">opus</SelectItem>
              <SelectItem value="sonnet">sonnet</SelectItem>
              <SelectItem value="haiku">haiku</SelectItem>
            </SelectContent>
          </Select>
        </section>

        <section className="flex flex-col gap-2">
          <label className="text-fg-tertiary text-[10px] font-semibold uppercase tracking-[0.12em]">
            Slider
          </label>
          <div className="flex items-center gap-3">
            <Slider
              value={value}
              onValueChange={setValue}
              min={50}
              max={100}
              step={1}
              className="flex-1"
            />
            <span className="font-mono text-xs">{value[0]}%</span>
          </div>
        </section>

        <section className="flex flex-col gap-2">
          <label className="text-fg-tertiary text-[10px] font-semibold uppercase tracking-[0.12em]">
            Dialog
          </label>
          <Dialog>
            <DialogTrigger asChild>
              <Button variant="secondary">Open dialog</Button>
            </DialogTrigger>
            <DialogContent>
              <DialogHeader>
                <DialogTitle>Sample dialog</DialogTitle>
              </DialogHeader>
              <div className="p-4 text-xs text-fg-secondary">
                Modal body content here.
              </div>
            </DialogContent>
          </Dialog>
        </section>
      </div>
    </main>
  )
}
