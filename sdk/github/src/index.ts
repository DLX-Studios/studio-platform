/** Host-mediated request descriptor used by first-party integration SDKs. */
export interface RestRequest {
  readonly method: "GET" | "POST";
  readonly origin: string;
  readonly path: string;
  readonly query?: string;
  readonly headers?: Readonly<Record<string, string>>;
  readonly body?: unknown;
}

/** The only host capability the SDK needs; credentials are injected outside this interface. */
export interface GithubRestClient {
  request<T>(request: RestRequest): Promise<T>;
}

export const GITHUB_PROVIDER = "github" as const;
export const GITHUB_API_ORIGIN = "https://api.github.com" as const;
export const GITHUB_DESCRIPTOR_VERSION = "1.0.0" as const;

/** Version-pinned integration configuration emitted into a signed application package. */
export interface GithubIntegration {
  readonly id: typeof GITHUB_PROVIDER;
  readonly version: typeof GITHUB_DESCRIPTOR_VERSION;
  readonly clientId: string;
  readonly clientSecretName: string;
  readonly scopes: readonly ["read:user", "user:email", "repo"];
}

/** GitHub route declarations; the host rejects every path outside this set. */
export const GITHUB_ROUTES = [
  { id: "github.user", method: "GET", path: "/user" },
  { id: "github.repositories", method: "GET", path: "/user/repos" },
  { id: "github.repository", method: "GET", path: "/repos/{owner}/{repo}" },
] as const;

export interface GithubUser {
  readonly id: number;
  readonly login: string;
  readonly name?: string | null;
  readonly email?: string | null;
  readonly avatarUrl?: string | null;
}

export interface GithubRepository {
  readonly id: number;
  readonly owner: string;
  readonly name: string;
  readonly fullName: string;
  readonly description?: string | null;
  readonly private: boolean;
  readonly htmlUrl: string;
  readonly defaultBranch?: string | null;
  readonly stars: number;
  readonly forks: number;
}

export interface GithubRepositoryDetail {
  readonly repository: GithubRepository;
  readonly openIssues: number;
  readonly language?: string | null;
  readonly updatedAt?: string | null;
}

/** Typed GitHub client. It owns no token and performs no unrestricted network operation. */
export class GithubClient {
  public constructor(private readonly rest: GithubRestClient) {}

  public currentUser(): Promise<GithubUser> {
    return this.rest.request<Record<string, unknown>>(this.request("/user")).then(parseUser);
  }

  public async repositories(): Promise<readonly GithubRepository[]> {
    const response = await this.rest.request<ReadonlyArray<Record<string, unknown>>>(
      this.request("/user/repos", "sort=updated&direction=desc&per_page=50"),
    );
    return response.map(parseRepository);
  }

  public async repository(owner: string, name: string): Promise<GithubRepositoryDetail> {
    if (!safeSegment(owner) || !safeSegment(name)) throw new Error("github repository reference invalid");
    const value = await this.rest.request<Record<string, unknown>>(this.request(`/repos/${owner}/${name}`));
    const repository = parseRepository(value);
    return {
      repository,
      openIssues: number(value.open_issues_count),
      language: optionalString(value.language),
      updatedAt: optionalString(value.updated_at),
    };
  }

  private request(path: string, query?: string): RestRequest {
    return {
      method: "GET",
      origin: GITHUB_API_ORIGIN,
      path,
      query,
      headers: {
        accept: "application/vnd.github+json",
        "x-github-api-version": "2022-11-28",
        "user-agent": "studio-github-sdk/0.1",
      },
    };
  }
}

function parseUser(value: Record<string, unknown>): GithubUser {
  return {
    id: number(value.id),
    login: string(value.login),
    name: optionalString(value.name),
    email: optionalString(value.email),
    avatarUrl: optionalString(value.avatar_url),
  };
}

function parseRepository(value: Record<string, unknown>): GithubRepository {
  const owner = value.owner;
  if (!owner || typeof owner !== "object") throw new Error("github response projection invalid");
  const ownerLogin = (owner as Record<string, unknown>).login;
  if (typeof ownerLogin !== "string") throw new Error("github response projection invalid");
  return {
    id: number(value.id),
    owner: ownerLogin,
    name: string(value.name),
    fullName: string(value.full_name),
    description: optionalString(value.description),
    private: typeof value.private === "boolean" ? value.private : false,
    htmlUrl: string(value.html_url),
    defaultBranch: optionalString(value.default_branch),
    stars: number(value.stargazers_count, 0),
    forks: number(value.forks_count, 0),
  };
}

function string(value: unknown): string {
  if (typeof value !== "string") throw new Error("github response projection invalid");
  return value;
}

function number(value: unknown, fallback?: number): number {
  if (typeof value === "number" && Number.isSafeInteger(value)) return value;
  if (fallback !== undefined) return fallback;
  throw new Error("github response projection invalid");
}

function optionalString(value: unknown): string | null | undefined {
  if (value === undefined || value === null) return value as null | undefined;
  return string(value);
}

function safeSegment(value: string): boolean {
  return value.length > 0 && value.length <= 100 && /^[A-Za-z0-9_.-]+$/.test(value);
}
