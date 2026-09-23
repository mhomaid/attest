"use client";

import { AnimatePresence, motion } from "framer-motion";
import { useState } from "react";
import { deployments, planes, type Deployment } from "@/components/marketing/stack-data";
import { analytics } from "@/lib/analytics";
import { cn } from "@/lib/utils";

const controlPlanes = planes.filter((p) => p.group === "control");
const dataPlanes = planes.filter((p) => p.group === "data");

export function DeploymentSection() {
  const [active, setActive] = useState(deployments[0].id);
  const model = deployments.find((d) => d.id === active) ?? deployments[0];

  return (
    <section id="deploy" className="scroll-mt-16 border-b border-border/60 py-20">
      <div className="mx-auto max-w-6xl px-4 sm:px-6">
        <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-primary">
          Control plane · data plane
        </p>
        <h2 className="mt-3 max-w-3xl text-2xl font-semibold tracking-tight sm:text-3xl">
          Same six planes. You choose where they run.
        </h2>
        <p className="mt-3 max-w-2xl text-sm text-muted-foreground">
          We can host them, or the data plane can live in your VPC. Same containers either way.
        </p>

        <div className="mt-8 flex flex-wrap gap-1.5">
          {deployments.map((d) => (
            <button
              key={d.id}
              type="button"
              onClick={() => {
                setActive(d.id);
                analytics.marketing_demo_step("deploy", d.id);
              }}
              aria-current={active === d.id ? "true" : undefined}
              className={cn(
                "rounded-full border px-3 py-1.5 font-mono text-[10px] uppercase tracking-wider transition-colors",
                active === d.id
                  ? "border-primary/50 bg-primary/10 text-primary"
                  : "border-border/70 text-muted-foreground hover:border-border",
              )}
            >
              {d.name}
              {d.status === "planned" ? (
                <span className="ml-1.5 text-muted-foreground">· planned</span>
              ) : (
                <span className="ml-1.5 text-signal-good">· live</span>
              )}
            </button>
          ))}
        </div>

        <div className="mt-6 grid gap-3 md:grid-cols-2">
          <CloudBox
            title="Attest cloud"
            sides={model}
            where="attest"
          />
          <CloudBox
            title="Your cloud / VPC"
            sides={model}
            where="customer"
          />
        </div>

        <AnimatePresence mode="wait">
          <motion.p
            key={model.id}
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            className="mt-4 text-sm text-muted-foreground"
          >
            {model.note}
          </motion.p>
        </AnimatePresence>
      </div>
    </section>
  );
}

function CloudBox({
  title,
  sides,
  where,
}: {
  title: string;
  sides: Deployment;
  where: "attest" | "customer";
}) {
  const controlHere = sides.controlIn === where;
  const dataHere = sides.dataIn === where;
  const empty = !controlHere && !dataHere;

  return (
    <div className="rounded-2xl border border-border/70 bg-card/30 p-4 sm:p-5">
      <p className="font-mono text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
        {title}
      </p>
      <div className="mt-4 min-h-[12rem]">
        {empty ? (
          <p className="py-10 text-center text-sm text-muted-foreground">
            {where === "customer"
              ? "Not used — we host all six planes."
              : "Not used — everything runs on your hardware."}
          </p>
        ) : (
          <div className="space-y-3">
            {controlHere ? <PlaneGroup label="Control plane" items={controlPlanes} /> : null}
            {dataHere ? <PlaneGroup label="Data plane" items={dataPlanes} /> : null}
          </div>
        )}
      </div>
    </div>
  );
}

function PlaneGroup({
  label,
  items,
}: {
  label: string;
  items: typeof planes;
}) {
  return (
    <div>
      <p className="mb-1.5 font-mono text-[10px] uppercase tracking-wider text-primary">{label}</p>
      <ul className="flex flex-wrap gap-1.5">
        {items.map((p) => (
          <motion.li
            key={p.id}
            layout
            initial={{ opacity: 0, scale: 0.96 }}
            animate={{ opacity: 1, scale: 1 }}
            className="rounded-md border border-border/70 bg-background/60 px-2.5 py-1 text-xs font-semibold"
          >
            {p.name}
          </motion.li>
        ))}
      </ul>
    </div>
  );
}
