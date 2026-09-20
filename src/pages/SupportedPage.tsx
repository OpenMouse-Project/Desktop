import { useState } from "preact/hooks";
import { Search, X } from "lucide-preact";
import type { SupportedBrand } from "../lib/supported-mice";
import { SUPPORTED_BRANDS } from "../lib/supported-mice";

export function SupportedPage() {
  const [query, setQuery] = useState("");
  const [selectedBrand, setSelectedBrand] = useState(SUPPORTED_BRANDS[0].brand);
  const total = SUPPORTED_BRANDS.reduce((count, brand) => count + brand.models.length, 0);
  const normalizedQuery = query.trim().toLocaleLowerCase();

  const matches = SUPPORTED_BRANDS.map((brand): SupportedBrand => {
    if (!normalizedQuery || brand.brand.toLocaleLowerCase().includes(normalizedQuery)) {
      return brand;
    }

    return {
      ...brand,
      models: brand.models.filter((mouse) =>
        `${mouse.model} ${mouse.note ?? ""}`.toLocaleLowerCase().includes(normalizedQuery),
      ),
    };
  }).filter((brand) => brand.models.length > 0);

  const resultTotal = matches.reduce((count, brand) => count + brand.models.length, 0);
  const activeBrand = matches.find((brand) => brand.brand === selectedBrand) ?? matches[0];

  return (
    <section class="page page-supported">
      <header class="supported-header">
        <div>
          <span class="supported-kicker">Compatibility</span>
          <h1>Supported mice</h1>
          <p>
            {total} models across {SUPPORTED_BRANDS.length} brands. Connect one and OpenMouse
            detects it automatically.
          </p>
        </div>

        <label class="supported-search">
          <Search size={15} aria-hidden="true" />
          <span class="supported-sr-only">Search supported mice</span>
          <input
            type="search"
            value={query}
            onInput={(event) => setQuery(event.currentTarget.value)}
            placeholder="Search models or brands"
          />
          {query ? (
            <button
              type="button"
              class="supported-search-clear"
              aria-label="Clear search"
              onClick={() => setQuery("")}
            >
              <X size={13} aria-hidden="true" />
            </button>
          ) : null}
        </label>
      </header>

      {activeBrand ? (
        <div class="supported-browser">
          <aside class="supported-brand-rail" aria-label="Mouse brands">
            <div class="supported-brand-rail-heading">
              <span>Brands</span>
              <span>{normalizedQuery ? `${resultTotal} results` : SUPPORTED_BRANDS.length}</span>
            </div>
            <div class="supported-brand-options">
              {matches.map((brand) => {
                const active = brand.brand === activeBrand.brand;
                return (
                  <button
                    type="button"
                    class={`supported-brand-option ${active ? "active" : ""}`}
                    aria-pressed={active}
                    onClick={() => setSelectedBrand(brand.brand)}
                    key={brand.brand}
                  >
                    <span>{brand.brand}</span>
                    <span>{brand.models.length}</span>
                  </button>
                );
              })}
            </div>
          </aside>

          <div class="supported-brand-detail">
            <div class="supported-brand-detail-header">
              <div>
                <h2>{activeBrand.brand}</h2>
                <p>
                  {activeBrand.models.length} supported{" "}
                  {activeBrand.models.length === 1 ? "model" : "models"}
                </p>
              </div>
              <span class="supported-driver-status">
                <span aria-hidden="true" />
                Driver available
              </span>
            </div>

            <div class="supported-model-grid">
              {activeBrand.models.map((mouse) => (
                <article class="supported-model-card" key={mouse.model}>
                  <div class="supported-model-copy">
                    <h3>{mouse.model}</h3>
                    {mouse.note ? <p>{mouse.note}</p> : null}
                  </div>
                </article>
              ))}
            </div>
          </div>
        </div>
      ) : (
        <div class="supported-empty">
          <h2>No matches</h2>
          <p>Try another model or brand.</p>
          <button type="button" onClick={() => setQuery("")}>
            Clear search
          </button>
        </div>
      )}
    </section>
  );
}