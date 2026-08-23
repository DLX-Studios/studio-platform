import { Money } from "./money";

export class Product {
  constructor(
    public id: string,
    public name: string,
    public category: string,
    public price: Money,
    public available: bool,
    public asset: string
  ) {}

  matches(query: string): bool {
    if (query.length == 0) return true;
    const q = query.toLowerCase();
    return this.name.toLowerCase().includes(q) ||
      this.category.toLowerCase().includes(q) ||
      this.price.format().toLowerCase().includes(q);
  }

  matchesCategory(category: string): bool {
    if (category == "All") return true;
    return this.category == category;
  }
}

// 12 dishes matching Pospay reference + uploaded assets
export const BUTTER_CHICKEN = new Product("butter-chicken", "Butter Chicken", "Main Course", Money.fromCents(1264), true, "assets/dishes/buttery-chicken.webp");
export const FRENCH_FRIES = new Product("french-fries", "French Fries", "Main Course", Money.fromCents(750), true, "assets/dishes/french-fries.webp");
export const ROAST_BEEF = new Product("roast-beef", "Roast Beef", "Main Course", Money.fromCents(2900), true, "assets/dishes/roast-beef.webp");
export const SAUERKRAUT = new Product("sauerkraut", "Sauerkraut", "Main Course", Money.fromCents(1155), true, "assets/dishes/sauerkraut.webp");
export const BEEF_KEBAB = new Product("beef-kebab", "Beef Kebab", "Main Course", Money.fromCents(1495), false, "assets/dishes/beef-kebab.webp");
export const FISH_CHIPS = new Product("fish-chips", "Fish and Chips", "Dessert", Money.fromCents(2305), true, "assets/dishes/fish-chips.webp");
export const WAGYU = new Product("wagyu", "Wagyu Steak", "Appetizer", Money.fromCents(3117), true, "assets/dishes/wagyu.webp");
export const CHICKEN_RAMEN = new Product("chicken-ramen", "Chicken Ramen", "Appetizer", Money.fromCents(1770), true, "assets/dishes/chicken-ramen.webp");
export const PASTA = new Product("pasta", "Pasta Bolognese", "Main Course", Money.fromCents(2350), true, "assets/dishes/spaghetti-bolognese.webp");
export const VEG_SALAD = new Product("veg-salad", "Vegetable Salad", "Beverages", Money.fromCents(1541), true, "assets/dishes/veggie-salad.webp");
export const GRILLED_SKEWERS = new Product("grilled-skewers", "Grilled Skewers", "Dessert", Money.fromCents(1725), false, "assets/dishes/grilled-skewers.webp");
export const FRIED_RICE = new Product("fried-rice", "Fried Rice", "Beverages", Money.fromCents(1950), true, "assets/dishes/fried-rice.webp");

export const ALL_PRODUCTS: Product[] = [
  BUTTER_CHICKEN, FRENCH_FRIES, ROAST_BEEF, SAUERKRAUT,
  BEEF_KEBAB, FISH_CHIPS, WAGYU, CHICKEN_RAMEN,
  PASTA, VEG_SALAD, GRILLED_SKEWERS, FRIED_RICE
];

// Pospay pill tabs — counts reflect real data (not fake 43)
export const CATEGORIES: string[] = ["All", "Beverages", "Main Course", "Dessert", "Appetizer"];

export function countFor(cat: string): i32 {
  if (cat == "All") return ALL_PRODUCTS.length;
  let n: i32 = 0;
  for (let i = 0; i < ALL_PRODUCTS.length; i++) if (ALL_PRODUCTS[i].category == cat) n++;
  return n;
}

export function productById(id: string): Product | null {
  for (let i = 0; i < ALL_PRODUCTS.length; i++) if (ALL_PRODUCTS[i].id == id) return ALL_PRODUCTS[i];
  return null;
}
