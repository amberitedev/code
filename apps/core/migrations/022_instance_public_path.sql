ALTER TABLE instances ADD COLUMN path TEXT;

WITH ranked AS (
	SELECT
		id,
		COALESCE(
			NULLIF(
				TRIM(
					REPLACE(
						REPLACE(
							REPLACE(
								REPLACE(
									REPLACE(
										REPLACE(
											REPLACE(
												REPLACE(
													REPLACE(
														REPLACE(
															REPLACE(name, '/', '_'),
															CHAR(92), '_'
														),
														'?', '_'
													),
													'*', '_'
												),
												':', '_'
											),
											CHAR(39), '_'
										),
										CHAR(34), '_'
									),
									'|', '_'
								),
								'<', '_'
							),
							'>', '_'
						),
						'!', '_'
					)
				),
				''
			),
			'server'
		) AS base_path,
		ROW_NUMBER() OVER (
			PARTITION BY COALESCE(
				NULLIF(
					TRIM(
						REPLACE(
							REPLACE(
								REPLACE(
									REPLACE(
										REPLACE(
											REPLACE(
												REPLACE(
													REPLACE(
														REPLACE(
															REPLACE(
																REPLACE(name, '/', '_'),
																CHAR(92), '_'
															),
															'?', '_'
														),
														'*', '_'
													),
													':', '_'
												),
												CHAR(39), '_'
											),
											CHAR(34), '_'
										),
										'|', '_'
									),
									'<', '_'
								),
								'>', '_'
							),
							'!', '_'
						)
					),
					''
				),
				'server'
			)
			ORDER BY created_at, id
		) AS path_index
	FROM instances
)
UPDATE instances
SET path = (
	SELECT CASE
		WHEN ranked.path_index = 1 THEN ranked.base_path
		ELSE ranked.base_path || ' (' || (ranked.path_index - 1) || ')'
	END
	FROM ranked
	WHERE ranked.id = instances.id
);

CREATE UNIQUE INDEX idx_instances_path ON instances(path);
