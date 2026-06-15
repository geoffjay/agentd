/**
 * Types for the product-admin (superuser) views. These mirror the core
 * service's `/api/v1/admin/*` responses — read-only, product-wide, and never
 * including secrets (no password_hash, no session token_hash).
 */

export interface AdminUser {
	id: string;
	username: string | null;
	email: string;
	display_name: string | null;
	role: string;
	is_superuser: boolean;
	active_organization_id: string | null;
	created_at: string;
	updated_at: string;
}

export interface AdminOrganization {
	id: string;
	name: string;
	slug: string;
	created_at: string;
	updated_at: string;
}

export interface AdminMembership {
	id: string;
	user_id: string;
	organization_id: string;
	role: string;
	created_at: string;
	updated_at: string;
}

export interface AdminSession {
	id: string;
	user_id: string;
	expires_at: string;
	is_expired: boolean;
	created_at: string;
}
