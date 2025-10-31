# 태스크 가이드: User 기능의 `name`을 `username`으로 변경하기

## Rust 기초 입문

### 알아야 할 Rust 기본 개념
- **소유권(Ownership)**: Rust는 데이터를 누가 "소유"하는지 추적합니다. 함수에 데이터를 전달하면 소유권이 이동하는데, `&`(빌림)을 사용하면 이동하지 않습니다.
- **구조체(Structs)**: TypeScript 인터페이스나 Python 클래스와 비슷하지만, 상속은 없습니다. 예시:
  ```rust
  pub struct User {
      pub id: LazySnowflake,
      pub name: String,  // ← 여기를 바꾸게 됩니다!
  }
  ```
- **트레이트(Traits)**: TypeScript 인터페이스나 Python 프로토콜과 유사 - 행동 규약을 정의합니다.
- **Option<T>**: TypeScript의 `T | null`이나 Python의 `Optional[T]`와 같습니다. `Some(value)` 또는 `None`을 사용합니다.
- **Result<T, E>**: 실패할 수 있는 함수에 사용합니다. `Ok(value)` 또는 `Err(error)`를 사용합니다.
- **매크로(Macros)**: 코드를 생성하는 코드로, `!`로 끝납니다. 예: `println!()`, `sqlx::query!()`

### 이 코드베이스의 일반적인 패턴
- `pub` = public (다른 모듈에서 접근 가능)
- `async fn` = 비동기 함수 (TypeScript의 `async function`과 같음)
- `.await?` = 결과를 await하고 에러를 위로 전파 (TypeScript의 `await` + `throw`)
- `&self` = self를 빌림 (Python의 `self`와 유사)
- `String` vs `&str`: `String`은 소유, `&str`은 빌림(참조와 같음)

## HQ 코드베이스 아키텍처

HQ는 **클린 아키텍처** 패턴을 따르며, 관심사를 명확히 분리합니다:

```
src/
├── feature/          # 도메인별로 구성된 비즈니스 로직
│   ├── user/         # ← 여기서 작업하게 됩니다
│   │   ├── mod.rs    # User 구조체 정의
│   │   ├── types.rs  # 요청/응답 DTO (CreateUser, UpdateUserInfo 등)
│   │   ├── service.rs    # 비즈니스 로직 레이어
│   │   ├── repository.rs # 데이터 접근 트레이트 (인터페이스)
│   │   └── error.rs  # 도메인별 에러
│   ├── auth/
│   └── ...
├── infrastructure/   # 리포지토리의 구체적인 구현
│   ├── postgres/
│   │   └── user.rs   # ← User를 위한 SQL 쿼리
│   └── redis/
├── controller/       # HTTP 레이어 (API 라우트)
│   └── routes/
│       └── user.rs   # ← User API 엔드포인트
├── core/             # 앱 초기화 및 의존성 주입
└── util/             # 공유 유틸리티
```

### 주요 아키텍처 개념

**1. 기능 모듈** (`feature/user/`)
- **Types** (`types.rs`): API 요청을 위한 DTO (CreateUser, UpdateUserInfo)
- **Service** (`service.rs`): 비즈니스 로직 조율
- **Repository** (`repository.rs`): 데이터 접근 인터페이스를 정의하는 트레이트 (TS 인터페이스와 유사)
- **Errors** (`error.rs`): 도메인별 에러 타입

**2. 인프라 레이어** (`infrastructure/postgres/user.rs`)
- 리포지토리 트레이트를 구현
- 실제 SQL 쿼리 포함
- 데이터베이스 특화 로직과 에러 매핑 처리

**3. 컨트롤러 레이어** (`controller/routes/user.rs`)
- HTTP 엔드포인트 핸들러
- `axum` 웹 프레임워크 사용 (Node의 Express.js와 유사)
- 권한 검증 및 서비스 호출

**4. 의존성 주입** (`core/app.rs`)
- 서비스와 의존성을 연결
- AppState가 설정과 서비스 인스턴스를 보관

### 데이터 흐름

```
HTTP 요청 → 컨트롤러 → 서비스 → 리포지토리 → 데이터베이스
                ↓         ↓         ↓
            (검증)   (비즈니스)  (SQL 쿼리)
```

예시: 사용자 생성
1. `POST /api/v1/user` → `controller/routes/user.rs::create_user()`
2. 컨트롤러가 권한 검증
3. `service.user_service.create_user(CreateUser)` 호출
4. 서비스가 ID 생성 및 User 구조체 생성
5. `user_repo.create_user(User)` 호출 (트레이트 메서드)
6. PostgresDb가 SQL INSERT로 구현
7. 응답이 위로 전달됨

## 여러분의 태스크: `name` → `username` 변경하기

User 기능 전체에서 `name` 필드를 `username`으로 변경해야 합니다. 업데이트할 내용은 다음과 같습니다:

### 수정할 파일들

**1. 핵심 타입 정의**
- `src/feature/user.rs`: User 구조체 자체 (14번째 줄)

**2. 데이터베이스 레이어**
- `migrations/20251003225658_user.up.sql`: 데이터베이스 스키마
- `src/infrastructure/postgres/user.rs`: SQL 쿼리 및 컬럼 이름

**3. 비즈니스 로직**
- `src/feature/user/types.rs`: CreateUser와 UpdateUserInfo DTO
- `src/feature/user/service.rs`: 필드를 사용하는 서비스 로직
- `src/feature/user/repository.rs`: UpdateUser 구조체

**4. 테스트**
- `tests/user.rs`: 테스트 단언문(assertion)

### 단계별 접근법

**1단계: 핵심 구조체 업데이트**
```rust
// src/feature/user.rs에서
pub struct User {
    pub id: LazySnowflake,
    pub username: String,  // 'name'에서 변경
    pub permissions: PermissionFlags,
}
```

**2단계: DTO 업데이트**
```rust
// src/feature/user/types.rs에서
pub struct CreateUser {
    pub username: String,  // 'name'에서 변경
    // ...
}

pub struct UpdateUserInfo {
    pub username: Option<String>,  // 'name'에서 변경
}
```

**3단계: 리포지토리 업데이트**
```rust
// src/feature/user/repository.rs에서
pub struct UpdateUser {
    pub username: Option<String>,  // 'name'에서 변경
    // ...
}
```

**4단계: 서비스 로직 업데이트**
```rust
// src/feature/user/service.rs에서
// 사용자 생성 시:
let user = User {
    id: user_id,
    username: data.username,  // 'name'에서 변경
    permissions: data.permissions,
};

// 업데이트 시:
UpdateUser {
    username: data.username,  // 'name'에서 변경
    permissions: None,
}
```

**5단계: 데이터베이스 레이어 업데이트**

먼저, 새 마이그레이션 생성:
```bash
sqlx migrate add rename_user_name_to_username
```

새 마이그레이션 파일(`.up.sql`)에:
```sql
ALTER TABLE users RENAME COLUMN name TO username;
```

롤백용(`.down.sql`)에:
```sql
ALTER TABLE users RENAME COLUMN username TO name;
```

그 다음 `src/infrastructure/postgres/user.rs` 업데이트:
```rust
// SQL 쿼리 변경:
"INSERT INTO users (id, username, permissions) ..."  // 'name'이었음
.bind(&data.username)  // 'data.name'이었음

// update_user에서:
if let Some(ref username) = data.username {  // 'name'이었음
    handle_sep!();
    qb.push("username = ").push_bind(username);  // 'name'이었음
}

// find_user에서:
let ident = User {
    id: id.into(),
    username: item.try_get("username")?,  // 'name'이었음
    permissions: ...
};
```

에러 매핑도 업데이트:
```rust
// map_user_query_error에서:
if pg_err.constraint() == Some("users_username_key") {  // 'users_name_key'였음
    return AppError::Business(BusinessError::User(UserError::DuplicateName));
}
```

**6단계: 테스트 업데이트**
```rust
// tests/user.rs에서
let ident = User {
    id: id.as_lazy(),
    username: "hi".into(),  // 'name'이었음
    permissions: perm.clone(),
};

// 단언문에서:
assert_eq!(ident_found.username, "hi".to_string());  // 'name'이었음
```

### 검증 체크리스트

변경 후:

1. **프로젝트 빌드**: `cargo build`
   - 타입 에러와 컴파일 이슈를 확인합니다

2. **테스트 실행**: `cargo test`
   - 변경사항이 제대로 작동하는지 확인합니다

3. **데이터베이스 확인**:
   - PostgreSQL이 실행 중인지 확인
   - 마이그레이션 실행: `sqlx migrate run`

4. **컴파일 에러 확인**:
   - Rust 컴파일러는 매우 도움이 됩니다!
   - 에러 메시지를 주의깊게 읽으세요 - 정확히 무엇이 잘못되었는지 알려줍니다

### 흔한 함정들

1. **데이터베이스 제약조건 이름을 잊어버림**: SQL 에러 처리의 unique 제약조건 이름
2. **검증 함수 이름**: `validate_username` 함수는 그대로 두어도 됩니다
3. **SQL 컬럼 이름**: 데이터베이스 스키마와 정확히 일치해야 합니다
4. **Option vs 직접 필드**: `UpdateUser`는 `Option<String>`을 사용하지만, `User`는 `String`을 사용합니다

### 성공을 위한 팁

- **컴파일러가 안내하게 하세요**: `cargo build`를 자주 실행하세요. Rust 컴파일러의 에러는 매우 도움이 됩니다.
- **에디터의 LSP 사용**: rust-analyzer가 타이핑하면서 에러를 보여줍니다
- **파일 전체 검색**: `rg username`을 사용해서 놓친 부분을 찾으세요
- **한 번에 한 파일씩**: 체계적으로 변경하고, 서두르지 마세요
- **기존 패턴 읽기**: 코드베이스는 일관성이 있습니다 - 보이는 패턴을 따르세요

### 에러 흐름 이해하기

사용자가 중복된 username을 만들려고 할 때:
```
1. PostgresDb.create_user()가 SQL INSERT 실행
2. 데이터베이스가 거부 (unique 제약조건 위반)
3. map_user_query_error()가 에러를 잡음
4. 제약조건 이름 확인: "users_username_key"  ← 이것을 업데이트하세요!
5. UserError::DuplicateName 반환
6. 컨트롤러가 400 Bad Request 반환
```

### 작업하면서 고려할 질문들

1. 왜 `User`는 `String`을 갖고 `UpdateUser`는 `Option<String>`을 가질까요?
   - `User`는 항상 username을 가지지만, 업데이트할 때는 변경하지 않을 수도 있습니다

2. 왜 `repository.rs` (트레이트)와 `postgres/user.rs` (구현)를 분리할까요?
   - 데이터베이스 구현을 교체할 수 있게 합니다 (예: MockUserRepository로 테스트)

3. `String`과 `&str`의 차이는 무엇인가요?
   - `String`은 데이터를 소유하고, `&str`은 빌립니다 (참조와 같음)

4. 왜 `async/await`를 사용하나요?
   - 데이터베이스 작업은 I/O 바운드이고, async는 많은 요청을 동시에 처리할 수 있게 합니다

행운을 빕니다! 기억하세요: Rust 컴파일러는 친구입니다. TypeScript나 Python에서는 런타임 에러가 될 버그를 컴파일 타임에 잡아줍니다.
