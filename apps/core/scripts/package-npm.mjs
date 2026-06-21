import { cp, mkdir, readFile, writeFile } from 'node:fs/promises'
import { basename, resolve } from 'node:path'

const [binaryPath, version] = process.argv.slice(2)

if (!binaryPath || !version) {
	throw new Error('Usage: node scripts/package-npm.mjs <copal-binary> <version>')
}

const root = resolve(import.meta.dirname, '../../..')
const packageDir = resolve(root, 'packages/copal-linux-x64')
const packageJsonPath = resolve(packageDir, 'package.json')
const packageJson = JSON.parse(await readFile(packageJsonPath, 'utf8'))

packageJson.version = version
await mkdir(resolve(packageDir, 'bin'), { recursive: true })
await cp(binaryPath, resolve(packageDir, 'bin', 'copal'))
await writeFile(packageJsonPath, `${JSON.stringify(packageJson, null, '\t')}\n`)

console.log(`Prepared ${packageJson.name}@${version} from ${basename(binaryPath)}`)
