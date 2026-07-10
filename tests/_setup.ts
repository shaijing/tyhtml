// Shared test setup. Module caching keeps the expensive TyHtml cold start
// to one instance for the complete test run.
import { TyHtml } from '../index'

// Module caching makes this a single cold start for the complete test run.
export const engine = new TyHtml()
