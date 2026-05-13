/** Format a decimal-as-string price with magnitude-aware precision. */
export function formatPrice(price: string): string {
  const n = Number(price);
  if (!Number.isFinite(n)) return price;
  const abs = Math.abs(n);
  let frac: number;
  if (abs === 0) frac = 2;
  else if (abs < 1) frac = 6;
  else if (abs < 10) frac = 4;
  else if (abs < 1000) frac = 2;
  else frac = 2;
  return n.toLocaleString(undefined, { minimumFractionDigits: frac, maximumFractionDigits: frac });
}

/** Format a decimal-as-string money amount with thousand separators (2 dp). */
export function formatMoney(amount: string): string {
  const n = Number(amount);
  if (!Number.isFinite(n)) return amount;
  return n.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}
