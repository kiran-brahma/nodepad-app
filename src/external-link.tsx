import type { AnchorHTMLAttributes } from "react"
import { thinkingWorkspace } from "./workspace-client"

/**
 * The ReactMarkdown `a` override. Nodepad's webview never navigates: every
 * anchor click is intercepted and, if the scheme is `http`/`https`, handed
 * to the macOS shell opener through the one durable seam. Non-HTTP(S)
 * schemes are rendered as inert text, so a Note can never open another app
 * or run a `javascript:` URL.
 *
 * The scheme is re-validated in Rust before anything opens; the frontend
 * check only decides whether the anchor is a link at all, so the predicate
 * of record lives in one place.
 */
function isOpenableScheme(href: string | undefined): boolean {
  if (!href) return false
  // Only an explicit, absolute http/https URL is a link here. Relative or
  // empty hrefs (react-markdown strips dangerous protocols to "") and every
  // other scheme are inert. The scheme is re-validated in Rust before
  // anything opens, so this only decides whether the anchor is a link at all.
  return /^https?:\/\//i.test(href)
}

export function ExternalLink({
  href,
  children,
  ...rest
}: AnchorHTMLAttributes<HTMLAnchorElement>) {
  if (!isOpenableScheme(href)) {
    // A link Nodepad will not open reads as text, not as a disabled control,
    // so it is never a focus trap and never a navigation surface.
    return <span className="external-link-rejected">{children}</span>
  }
  return (
    <a
      href={href}
      title={`Open ${href} in your browser`}
      onClick={(event) => {
        // The webview never handles the URL itself.
        event.preventDefault()
        void thinkingWorkspace.openExternalLink(href!)
      }}
      {...rest}
    >
      {children}
    </a>
  )
}