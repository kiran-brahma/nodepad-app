/** The shipped, privacy-enhanced Nodepad introduction. The CSP permits only
 * this frame origin; it never gives the webview general network access. */
export function IntroVideo() {
  return (
    <section aria-label="Nodepad introduction">
      <h2>Watch the introduction</h2>
      <iframe
        title="Nodepad introduction"
        src="https://www.youtube-nocookie.com/embed/jZu4sgZOOO4?rel=0&modestbranding=1&color=white"
        allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
        allowFullScreen
      />
    </section>
  )
}
