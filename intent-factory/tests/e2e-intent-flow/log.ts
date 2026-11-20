import { appendFileSync, mkdirSync } from "fs"
import { resolve } from "path"

// ---------------------------------------------------------------------------
// Per-run analysis logging
// ---------------------------------------------------------------------------

const runId = new Date().toISOString().replace(/[:.]/g, "-")
const analysisDir = resolve(
  process.cwd(),
  "tests/e2e-intent-flow/analysis",
  runId,
)
const analysisPath = resolve(analysisDir, "analysis.txt")

mkdirSync(analysisDir, { recursive: true })
appendFileSync(analysisPath, `Analysis started: ${new Date().toISOString()}\n`)
appendFileSync(analysisPath, `Working directory: ${process.cwd()}\n\n`)

function formatArg(arg: unknown): string {
  if (typeof arg === "string") return arg
  if (typeof arg === "bigint") return arg.toString()
  if (arg instanceof Error) return arg.stack ?? arg.message

  try {
    return JSON.stringify(
      arg,
      (_key, value: unknown) =>
        typeof value === "bigint" ? value.toString() : value,
      2,
    )
  } catch {
    return String(arg)
  }
}

function writeLine(args: unknown[]): string {
  const line = args.map(formatArg).join(" ")
  appendFileSync(analysisPath, `${line}\n`)
  return line
}

export function getAnalysisPath(): string {
  return analysisPath
}

export function log(...args: unknown[]): void {
  writeLine(args)
  console.log(...args)
}

export function logError(...args: unknown[]): void {
  writeLine(args)
  console.error(...args)
}
