declare function ToObject<T extends object>(value: number): number;
declare function ToObject<T extends object>(value: boolean): boolean;
declare function ToObject<T extends object>(value: string): string;
declare function ToObject<T extends object>(value: bigint): bigint;
declare function ToObject<T extends object>(value: T): T;

export = ToObject;
