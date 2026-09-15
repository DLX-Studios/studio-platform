declare module "*.yml?raw" {
  const value: string;
  export default value;
}

declare module "*.yml" {
  const value: string;
  export default value;
}
