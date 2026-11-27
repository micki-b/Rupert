// From, amount and due date (#108): what the forms and the From filter
// share. The senders in use (most documents first) for suggestions while
// typing, and common currency codes.
import { api, type TagCount } from "#lib/api.ts";

/** Offered while typing a currency; any three-letter code works. */
export const CURRENCIES = ["EUR", "USD", "GBP", "CHF", "CAD", "AUD", "MXN", "ARS", "BRL", "CLP", "COP", "JPY", "SEK", "NOK", "DKK", "PLN"];

class Senders {
  /** Senders with how many documents each, most first. */
  list = $state<TagCount[]>([]);
  names = $derived(this.list.map((s) => s.tag));
  #loading: Promise<void> | null = null;

  /** Reads them again from the index (after a save, or when it changed). */
  refresh(): Promise<void> {
    this.#loading = api
      .listSenders("")
      .then((l) => {
        this.list = l;
      })
      .catch(() => {});
    return this.#loading;
  }

  /** The first read, once. */
  ensure(): Promise<void> {
    return this.#loading ?? this.refresh();
  }
}

export const senderList = new Senders();
