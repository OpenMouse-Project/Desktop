import { SUPPORTED_BRANDS } from "../lib/supported-mice";

export function SupportedPage() {
  let total = 0;
  for (const b of SUPPORTED_BRANDS) total += b.models.length;

  return (
    <section class="page page-supported">
      <h1 class="page-title">Supported mice</h1>
      <p class="page-description">
        {total} models across {SUPPORTED_BRANDS.length} brands have a working driver. Plug any
        of them in and its live settings appear automatically — this list just
        shows what's covered without a device attached.
      </p>

      <div class="supported-brands">
        {SUPPORTED_BRANDS.map((brand) => (
          <div class="supported-brand" key={brand.brand}>
            <h2 class="supported-brand-name">{brand.brand}</h2>
            <ul class="supported-model-list">
              {brand.models.map((mouse) => (
                <li class="supported-model" key={mouse.model}>
                  <span class="supported-model-name">{mouse.model}</span>
                  {mouse.note ? <span class="supported-model-note">{mouse.note}</span> : null}
                </li>
              ))}
            </ul>
          </div>
        ))}
      </div>
    </section>
  );
}