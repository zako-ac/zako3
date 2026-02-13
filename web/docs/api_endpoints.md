# API Endpoints

This document outlines the API endpoints utilized by the web application.

## Authentication

| Method | Endpoint         | Description                                  |
| :----- | :--------------- | :------------------------------------------- |
| `GET`  | `/auth/login`    | Get login URL (e.g., for Discord OAuth)      |
| `GET`  | `/auth/callback` | Handle OAuth callback (expects `?code=...`)  |
| `POST` | `/auth/callback` | Handle OAuth callback via POST (alternative) |
| `POST` | `/auth/logout`   | Logout the current user                      |
| `GET`  | `/auth/refresh`  | Refresh the authentication token             |

## Users

| Method | Endpoint         | Description                                    |
| :----- | :--------------- | :--------------------------------------------- |
| `GET`  | `/users/me`      | Get current authenticated user's profile       |
| `GET`  | `/users/:userId` | Get public profile of a user                   |
| `GET`  | `/users/me/taps` | Get taps owned by the current user (paginated) |

## Taps

| Method   | Endpoint                 | Description                                         |
| :------- | :----------------------- | :-------------------------------------------------- |
| `GET`    | `/taps`                  | List taps (supports filtering, sorting, pagination) |
| `POST`   | `/taps`                  | Create a new tap                                    |
| `GET`    | `/taps/:tapId`           | Get details of a specific tap                       |
| `PATCH`  | `/taps/:tapId`           | Update a tap                                        |
| `DELETE` | `/taps/:tapId`           | Delete a tap                                        |
| `GET`    | `/taps/:tapId/stats`     | Get usage statistics for a tap                      |
| `GET`    | `/taps/:tapId/audit-log` | Get audit log for a tap                             |
| `POST`   | `/taps/:tapId/report`    | Report a tap                                        |
| `POST`   | `/taps/:tapId/verify`    | Request verification for a tap                      |

### Tap API Tokens

| Method   | Endpoint                                      | Description                       |
| :------- | :-------------------------------------------- | :-------------------------------- |
| `GET`    | `/taps/:tapId/api-tokens`                     | List API tokens for a tap         |
| `POST`   | `/taps/:tapId/api-tokens`                     | Create a new API token for a tap  |
| `PATCH`  | `/taps/:tapId/api-tokens/:tokenId`            | Update an API token (e.g., label) |
| `POST`   | `/taps/:tapId/api-tokens/:tokenId/regenerate` | Regenerate an API token           |
| `DELETE` | `/taps/:tapId/api-tokens/:tokenId`            | Delete an API token               |

## Notifications

| Method   | Endpoint                              | Description                         |
| :------- | :------------------------------------ | :---------------------------------- |
| `GET`    | `/notifications`                      | List notifications for current user |
| `GET`    | `/notifications/unread-count`         | Get count of unread notifications   |
| `PATCH`  | `/notifications/:notificationId/read` | Mark a notification as read         |
| `PATCH`  | `/notifications/read-all`             | Mark all notifications as read      |
| `DELETE` | `/notifications/:notificationId`      | Delete a notification               |

## Admin

| Method  | Endpoint                                  | Description                                        |
| :------ | :---------------------------------------- | :------------------------------------------------- |
| `GET`   | `/admin/users`                            | List all users (admin view, paginated)             |
| `GET`   | `/admin/users/:userId`                    | Get full user details (admin view)                 |
| `POST`  | `/admin/users/:userId/ban`                | Ban a user                                         |
| `POST`  | `/admin/users/:userId/unban`              | Unban a user                                       |
| `PATCH` | `/admin/users/:userId/role`               | Update user role (e.g. promote to admin)           |
| `GET`   | `/admin/notifications`                    | List notifications (admin view)                    |
| `GET`   | `/admin/activity`                         | Get admin activity logs                            |
| `GET`   | `/admin/taps/pending-verification`        | List taps pending verification                     |
| `GET`   | `/admin/verifications`                    | List verification requests (paginated, filterable) |
| `POST`  | `/admin/verifications/:requestId/approve` | Approve a verification request                     |
| `POST`  | `/admin/verifications/:requestId/reject`  | Reject a verification request                      |
